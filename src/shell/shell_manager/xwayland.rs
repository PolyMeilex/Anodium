use std::{
    cell::RefCell, collections::HashMap, convert::TryFrom, os::unix::net::UnixStream, rc::Rc,
    sync::Arc,
};

use smithay::{
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Client},
    utils::{x11rb::X11Source, Logical, Point},
    wayland::compositor::give_role,
};

use x11rb::{
    connection::Connection as _,
    errors::ReplyOrIdError,
    protocol::{
        composite::{ConnectionExt as _, Redirect},
        xproto::{
            ChangeWindowAttributesAux, ConfigWindow, ConfigureWindowAux, ConnectionExt as _,
            EventMask, Window, WindowClass,
        },
        Event,
    },
    rust_connection::{DefaultStream, RustConnection},
};

use crate::{desktop_layout::WindowSurface, state::BackendState};

impl BackendState {
    pub fn start_xwayland(&mut self) {
        if let Err(e) = self.xwayland.start() {
            error!("Failed to start XWayland: {}", e);
        }
    }
}

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_S0,
        WL_SURFACE_ID,
        _ANVIL_CLOSE_CONNECTION,
    }
}

/// The actual runtime state of the XWayland integration.
pub(super) struct X11State {
    conn: Arc<RustConnection>,
    atoms: Atoms,
    unpaired_surfaces: HashMap<u32, (Window, Point<i32, Logical>)>,

    on_new_window: Box<dyn FnMut(WindowSurface, /*location*/ Point<i32, Logical>)>,
}

impl X11State {
    pub fn start_wm<F>(
        connection: UnixStream,
        on_new_window: F,
    ) -> Result<(Self, X11Source), Box<dyn std::error::Error>>
    where
        F: FnMut(WindowSurface, Point<i32, Logical>) + 'static,
    {
        // Create an X11 connection. XWayland only uses screen 0.
        let screen = 0;
        let stream = DefaultStream::from_unix_stream(connection)?;
        let conn = RustConnection::connect_to_stream(stream, screen)?;
        let atoms = Atoms::new(&conn)?.reply()?;

        let screen = &conn.setup().roots[0];

        // Actually become the WM by redirecting some operations
        conn.change_window_attributes(
            screen.root,
            &ChangeWindowAttributesAux::default().event_mask(EventMask::SUBSTRUCTURE_REDIRECT),
        )?;

        // Tell XWayland that we are the WM by acquiring the WM_S0 selection. No X11 clients are accepted before this.
        let win = conn.generate_id()?;
        conn.create_window(
            screen.root_depth,
            win,
            screen.root,
            // x, y, width, height, border width
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &Default::default(),
        )?;
        conn.set_selection_owner(win, atoms.WM_S0, x11rb::CURRENT_TIME)?;

        // XWayland wants us to do this to function properly...?
        conn.composite_redirect_subwindows(screen.root, Redirect::MANUAL)?;

        conn.flush()?;

        let conn = Arc::new(conn);
        let wm = Self {
            conn: Arc::clone(&conn),
            atoms,
            unpaired_surfaces: Default::default(),

            on_new_window: Box::new(on_new_window),
        };

        Ok((
            wm,
            X11Source::new(
                conn,
                win,
                atoms._ANVIL_CLOSE_CONNECTION,
                slog_scope::logger(),
            ),
        ))
    }

    pub fn handle_event(&mut self, event: Event, client: &Client) -> Result<(), ReplyOrIdError> {
        debug!("X11: Got event {:?}", event);
        match event {
            Event::ConfigureRequest(r) => {
                // Just grant the wish
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
                self.conn.configure_window(r.window, &aux)?;
            }
            Event::MapRequest(r) => {
                // Just grant the wish
                self.conn.map_window(r.window)?;
            }
            Event::ClientMessage(msg) => {
                if msg.type_ == self.atoms.WL_SURFACE_ID {
                    // We get a WL_SURFACE_ID message when Xwayland creates a WlSurface for a
                    // window. Both the creation of the surface and this client message happen at
                    // roughly the same time and are sent over different sockets (X11 socket and
                    // wayland socket). Thus, we could receive these two in any order. Hence, it
                    // can happen that we get None below when X11 was faster than Wayland.

                    let location = {
                        match self.conn.get_geometry(msg.window)?.reply() {
                            Ok(geo) => (geo.x as i32, geo.y as i32).into(),
                            Err(err) => {
                                error!(
                                    "Failed to get geometry for {:x}, perhaps the window was already destroyed?",
                                    msg.window;
                                    "err" => format!("{:?}", err),
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
                            self.unpaired_surfaces.insert(id, (msg.window, location));
                        }
                        Some(surface) => self.new_window(msg.window, surface, location),
                    }
                }
            }
            _ => {}
        }
        self.conn.flush()?;
        Ok(())
    }

    fn new_window(&mut self, window: Window, surface: WlSurface, location: Point<i32, Logical>) {
        debug!("Matched X11 surface {:x?} to {:x?}", window, surface);

        if give_role(&surface, "x11_surface").is_err() {
            // It makes no sense to post a protocol error here since that would only kill Xwayland
            error!("Surface {:x?} already has a role?!", surface);
            return;
        }

        let x11surface = X11Surface {
            surface,
            window,
            conn: self.conn.clone(),
        };

        (self.on_new_window)(WindowSurface::X11(x11surface), location);
    }
}

// Called when a WlSurface commits.
pub(super) fn commit_hook(surface: &WlSurface) {
    // Is this the Xwayland client?
    if let Some(client) = surface.as_ref().client() {
        if let Some(x11) = client.data_map().get::<Rc<RefCell<X11State>>>() {
            let mut x11 = x11.borrow_mut();
            // Is the surface among the unpaired surfaces (see comment next to WL_SURFACE_ID
            // handling above)
            if let Some((window, location)) = x11.unpaired_surfaces.remove(&surface.as_ref().id()) {
                x11.new_window(window, surface.clone(), location);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct X11Surface {
    conn: Arc<RustConnection>,
    window: Window,
    surface: WlSurface,
}

impl std::cmp::PartialEq for X11Surface {
    fn eq(&self, other: &Self) -> bool {
        self.alive() && other.alive() && self.surface == other.surface
    }
}

impl X11Surface {
    pub fn alive(&self) -> bool {
        self.surface.as_ref().is_alive()
    }

    pub fn get_surface(&self) -> Option<&WlSurface> {
        if self.alive() {
            Some(&self.surface)
        } else {
            None
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        let aux = ConfigureWindowAux::default().width(width).height(height);
        self.conn.configure_window(self.window, &aux).ok();
        self.conn.flush().unwrap();
    }

    #[allow(dead_code)]
    pub fn move_to(&self, x: i32, y: i32) {
        let aux = ConfigureWindowAux::default().x(x).y(y);
        self.conn.configure_window(self.window, &aux).ok();
        self.conn.flush().unwrap();
    }
}
