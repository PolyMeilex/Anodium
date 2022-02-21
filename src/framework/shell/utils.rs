use smithay::{
    desktop::{self, Kind, PopupKind},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::shell::xdg::{self, ToplevelSurface},
};

pub trait AsWlSurface {
    fn as_surface(&self) -> Option<&WlSurface>;
}

impl AsWlSurface for WlSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        Some(self)
    }
}

impl AsWlSurface for desktop::Window {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.toplevel().get_surface()
    }
}

impl AsWlSurface for ToplevelSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}

impl AsWlSurface for xdg::PopupSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}

impl AsWlSurface for Kind {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}

impl AsWlSurface for PopupKind {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
    }
}
