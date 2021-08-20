use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface, wayland::shell::xdg::ToplevelSurface,
};

use crate::desktop_layout::Toplevel;

pub trait AsWlSurface {
    fn as_surface(&self) -> Option<&WlSurface>;
}

impl AsWlSurface for WlSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        Some(self)
    }
}

impl AsWlSurface for Toplevel {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}

impl AsWlSurface for ToplevelSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}
