use std::sync::Arc;

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use x11rb::{
    connection::Connection as _,
    protocol::xproto::{ConfigureWindowAux, ConnectionExt as _, Window},
    rust_connection::RustConnection,
};

#[derive(Clone, Debug)]
pub struct X11Surface {
    conn: Arc<RustConnection>,
    window: Window,
    surface: WlSurface,
}

impl X11Surface {
    pub fn new(conn: Arc<RustConnection>, window: Window, surface: WlSurface) -> Self {
        Self {
            conn,
            window,
            surface,
        }
    }
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
