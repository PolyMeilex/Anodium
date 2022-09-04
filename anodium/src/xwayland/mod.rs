use std::{collections::HashMap, convert::TryFrom, os::unix::net::UnixStream, time::Duration};

use crate::{data::seat::SeatState, positioning, CalloopData, State};
use calloop::{timer::Timer, LoopHandle};
use slog_scope::{debug, error};
use smithay::{
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Client, DisplayHandle, Resource},
    utils::{Logical, Point},
    xwayland::{XWayland, XWaylandEvent},
};
use x11rb::{
    connection::Connection,
    errors::ReplyOrIdError,
    protocol::{
        xproto::{ConfigWindow, ConfigureWindowAux, ConnectionExt, Window as X11Window},
        Event as X11Event,
    },
};

mod pending_window;
use pending_window::PendingWindow;

mod xwayland_client;
use xwayland_client::XWaylandClient;

#[derive(Debug)]
pub struct XWaylandState {
    xwayland_handle: XWayland,

    client: Option<XWaylandClient>,
    client_token: Option<calloop::RegistrationToken>,

    unpaired_surfaces: HashMap<u32, (X11Window, Point<i32, Logical>)>,
    windows_awaiting_map: Vec<PendingWindow>,
}

impl XWaylandState {
    /// init the xwayland connection
    pub fn init_xwayland_connection(
        handle: &LoopHandle<'static, CalloopData>,
        display: &DisplayHandle,
    ) -> Self {
        let (xwayland, channel) = XWayland::new(slog_scope::logger(), display);

        handle
            .insert_source(channel, {
                |event, _, data| {
                    XWaylandState::handle_xwayland_event(&mut data.state, event);
                }
            })
            .unwrap();

        Self {
            xwayland_handle: xwayland,
            client: None,
            client_token: None,
            unpaired_surfaces: Default::default(),
            windows_awaiting_map: Default::default(),
        }
    }

    /// Attempt to start the XWayland instance
    ///
    /// If it succeeds, we'll eventually receive an ready event
    pub fn start(&self, loop_handle: &LoopHandle<CalloopData>) {
        if let Err(e) = self.xwayland_handle.start(loop_handle.clone()) {
            error!("Failed to start XWayland: {}", e);
        }
    }

    fn ready(
        &mut self,
        loop_handle: &LoopHandle<CalloopData>,
        connection: UnixStream,
        client: Client,
    ) {
        let (client, source) = XWaylandClient::start(connection, client).unwrap();

        self.client = Some(client);

        let token = loop_handle
            .insert_source(source, |event, _, data| {
                if let Err(err) = handle_x11_event(&mut data.state, event) {
                    error!("Error while handling X11 event: {}", err);
                }
            })
            .ok();

        self.client_token = token;
    }

    fn exited(&mut self, loop_handle: &LoopHandle<CalloopData>) {
        self.client.take();
        self.unpaired_surfaces.clear();
        self.windows_awaiting_map.clear();

        if let Some(token) = self.client_token.take() {
            loop_handle.remove(token);
        }

        error!("Xwayland exited");

        // Restart after 1s
        let after = Timer::from_duration(Duration::from_secs(1));

        loop_handle
            .insert_source(after, |_, _, data| {
                data.state.xwayland.start(&data.state._loop_handle);
                calloop::timer::TimeoutAction::Drop
            })
            .ok();
    }

    fn handle_xwayland_event(state: &mut State, event: XWaylandEvent) {
        match event {
            XWaylandEvent::Ready {
                connection, client, ..
            } => {
                state
                    .xwayland
                    .ready(&state._loop_handle, connection, client);
            }
            XWaylandEvent::Exited => {
                state.xwayland.exited(&state._loop_handle);
            }
        }
    }
}

fn handle_x11_event(state: &mut State, event: X11Event) -> Result<(), ReplyOrIdError> {
    if let Some(client) = state.xwayland.client.clone() {
        debug!("X11: Got event {:?}", event);
        match event {
            X11Event::ConfigureRequest(r) => {
                let mut aux = ConfigureWindowAux::default();
                if r.value_mask & u16::from(ConfigWindow::STACK_MODE) != 0 {
                    aux = aux.stack_mode(r.stack_mode);
                }
                if r.value_mask & u16::from(ConfigWindow::SIBLING) != 0 {
                    aux = aux.sibling(r.sibling);
                }
                if r.value_mask & u16::from(ConfigWindow::X) != 0 {
                    aux = aux.x(i32::try_from(r.x).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::Y) != 0 {
                    aux = aux.y(i32::try_from(r.y).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::WIDTH) != 0 {
                    aux = aux.width(u32::try_from(r.width).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::HEIGHT) != 0 {
                    aux = aux.height(u32::try_from(r.height).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::BORDER_WIDTH) != 0 {
                    aux = aux.border_width(u32::try_from(r.border_width).unwrap());
                }

                client.conn.configure_window(r.window, &aux)?;
            }
            X11Event::MapRequest(r) => {
                client.conn.map_window(r.window)?;
            }
            X11Event::ClientMessage(msg) => {
                if msg.type_ == client.atoms.WL_SURFACE_ID {
                    let location = client
                        .conn
                        .get_geometry(msg.window)?
                        .reply()
                        .map(|geo| (geo.x as i32, geo.y as i32).into())
                        .unwrap_or_default();

                    let protocol_id = msg.data.as_data32()[0];
                    let surface = client
                        .wl_client
                        .object_from_protocol_id::<WlSurface>(&state.display, protocol_id);

                    match surface {
                        Ok(surface) => {
                            debug!(
                                "X11 surface {:x?} corresponds to WlSurface {:x} = {:?}",
                                msg.window, protocol_id, surface,
                            );

                            if let Some(window) = PendingWindow::new(msg.window, surface, location)
                            {
                                handle_new_window(state, window);
                            }
                        }
                        Err(_) => {
                            // X11 event was faster than wayland event,
                            // so we store the surface for latter use. once wayland event reaches us
                            state
                                .xwayland
                                .unpaired_surfaces
                                .insert(protocol_id, (msg.window, location));
                        }
                    }
                }
            }
            _ => {}
        }

        client.conn.flush()?;
    }

    Ok(())
}

fn on_window_map(state: &mut State, pending: PendingWindow) {
    let pointer_pos = SeatState::for_seat(&state.seat).pointer_pos();
    positioning::position_window_center(&mut state.space, pending.window, pointer_pos);
}

fn handle_new_window(state: &mut State, pending: PendingWindow) {
    if pending.is_buffer_attached() {
        on_window_map(state, pending);
    } else {
        state.xwayland.windows_awaiting_map.push(pending);
    }
}

// Called when a WlSurface commits.
pub fn handle_commit(state: &mut State, surface: &WlSurface) {
    if let Some(xwayland_client) = state.xwayland.client.clone() {
        // Check if new surface got mapped
        let pending = state
            .xwayland
            .windows_awaiting_map
            .iter()
            .enumerate()
            .find(|(_, pending)| pending.window.toplevel().wl_surface() == surface);

        if let Some((id, pending)) = pending {
            if pending.is_buffer_attached() {
                let pending = state.xwayland.windows_awaiting_map.remove(id);
                on_window_map(state, pending);
            }
        }

        if let Ok(client) = state.display.get_client(surface.id()) {
            // Is this the Xwayland client?
            if client == xwayland_client.wl_client {
                // Is the surface among the unpaired surfaces (see comment next to WL_SURFACE_ID
                // handling above)
                if let Some((window, location)) = state
                    .xwayland
                    .unpaired_surfaces
                    .remove(&surface.id().protocol_id())
                {
                    if let Some(window) = PendingWindow::new(window, surface.clone(), location) {
                        handle_new_window(state, window);
                    }
                }
            }
        }
    }
}
