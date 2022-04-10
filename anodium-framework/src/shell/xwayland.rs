use std::{cell::RefCell, os::unix::net::UnixStream, rc::Rc};

use smithay::{
    desktop::{Kind, X11Surface},
    reexports::{
        calloop::LoopHandle,
        wayland_server::{protocol::wl_surface::WlSurface, Client, DispatchData},
    },
    utils::{Logical, Point},
    wayland::compositor::give_role,
};

use x11rb::{
    connection::Connection as _,
    errors::ReplyOrIdError,
    protocol::{
        xproto::{ConnectionExt as _, Window},
        Event,
    },
};

mod x11_state;
use x11_state::X11State;

// mod x11_surface;
// pub use x11_surface::X11Surface;

use crate::shell::X11WindowUserData;

use super::ShellHandler;

impl<D> super::Inner<D>
where
    D: ShellHandler + 'static,
{
    pub fn xwayland_shell_event(
        &mut self,
        event: Event,
        x11: &mut X11State,
        client: &Client,
        handler: &mut D,
    ) -> Result<(), ReplyOrIdError> {
        debug!("X11: Got event {:?}", event);
        dbg!("X11: Got event {:?}", &event);
        match event {
            Event::ConfigureRequest(r) => {
                handler.xwayland_configure_request(x11.conn.clone(), r);
            }
            Event::MapRequest(r) => {
                // Just grant the wish
                x11.conn.map_window(r.window)?;
            }
            Event::ClientMessage(msg) => {
                if msg.type_ == x11.atoms.WL_SURFACE_ID {
                    // We get a WL_SURFACE_ID message when Xwayland creates a WlSurface for a
                    // window. Both the creation of the surface and this client message happen at
                    // roughly the same time and are sent over different sockets (X11 socket and
                    // wayland socket). Thus, we could receive these two in any order. Hence, it
                    // can happen that we get None below when X11 was faster than Wayland.

                    let location = {
                        match x11.conn.get_geometry(msg.window)?.reply() {
                            Ok(geo) => (geo.x as i32, geo.y as i32).into(),
                            Err(err) => {
                                error!(
                                    "Failed to get geometry for {:x}, perhaps the window was already destroyed? {}",
                                    msg.window,
                                    format!("{:?}", err),
                                );
                                (0, 0).into()
                            }
                        }
                    };

                    let id = msg.data.as_data32()[0];
                    let surface = client.get_resource::<WlSurface>(id);
                    info!(
                        "X11 surface {:x?} corresponds to WlSurface {:x} = {:?}",
                        msg.window, id, surface,
                    );
                    match surface {
                        None => {
                            x11.unpaired_surfaces.insert(id, (msg.window, location));
                        }
                        Some(surface) => {
                            self.new_window(x11, msg.window, surface.clone(), location);
                            self.try_map_unmaped(&surface, handler);
                        }
                    }
                }
            }
            _ => {}
        }
        x11.conn.flush()?;
        Ok(())
    }

    // Called when a WlSurface commits.
    pub fn xwayland_commit_hook(&mut self, surface: &WlSurface) {
        // Is this the Xwayland client?
        if let Some(client) = surface.as_ref().client() {
            if let Some(mut x11) = X11State::get_mut(&client) {
                // Is the surface among the unpaired surfaces (see comment next to WL_SURFACE_ID
                // handling above)
                if let Some((window, location)) =
                    x11.unpaired_surfaces.remove(&surface.as_ref().id())
                {
                    self.new_window(&x11, window, surface.clone(), location);
                }
            }
        }
    }

    fn new_window(
        &mut self,
        _x11: &X11State,
        window: Window,
        surface: WlSurface,
        location: Point<i32, Logical>,
    ) {
        debug!("Matched X11 surface {:x?} to {:x?}", window, surface);

        if give_role(&surface, "x11_surface").is_err() {
            // It makes no sense to post a protocol error here since that would only kill Xwayland
            error!("Surface {:x?} already has a role?!", surface);
            return;
        }

        // let x11surface = X11Surface::new(x11.conn.clone(), window, surface);
        let x11surface = X11Surface { surface };

        self.not_mapped_list.insert_window(
            Kind::X11(x11surface),
            Some(X11WindowUserData { window, location }),
        );
    }
}

pub fn xwayland_shell_init<F, D: 'static>(
    handle: &LoopHandle<D>,
    connection: UnixStream,
    client: Client,
    mut cb: F,
) where
    F: FnMut(Event, &mut X11State, &Client, DispatchData) + 'static,
{
    let (x11_state, source) = X11State::start_wm(connection).unwrap();

    let x11_state = Rc::new(RefCell::new(x11_state));
    client
        .data_map()
        .insert_if_missing(|| Rc::clone(&x11_state));

    handle
        .insert_source(source, move |event, _, ddata| {
            if let Some(mut x11) = X11State::get_mut(&client) {
                cb(event, &mut x11, &client, DispatchData::wrap(ddata));
            }
        })
        .unwrap();
}
