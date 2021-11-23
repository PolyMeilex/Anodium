use std::sync::Mutex;

use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::with_states,
        shell::xdg::{PopupSurface as XdgPopupSurface, XdgPopupSurfaceRoleAttributes},
    },
};

#[derive(Clone, Debug, PartialEq)]
pub enum PopupSurface {
    Xdg(XdgPopupSurface),
}

impl PopupSurface {
    pub fn dismiss(&self) {
        match *self {
            Self::Xdg(ref t) => t.send_popup_done(),
        }
    }

    pub fn alive(&self) -> bool {
        match *self {
            Self::Xdg(ref t) => t.alive(),
        }
    }

    pub fn get_surface(&self) -> Option<&wl_surface::WlSurface> {
        match *self {
            Self::Xdg(ref t) => t.get_surface(),
        }
    }

    pub fn parent(&self) -> Option<wl_surface::WlSurface> {
        let wl_surface = self.get_surface()?;

        with_states(wl_surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .parent
                .clone()
        })
        .ok()
        .flatten()
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        let wl_surface = match self.get_surface() {
            Some(s) => s,
            None => return Default::default(),
        };
        with_states(wl_surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .current
                .geometry
        })
        .unwrap_or_default()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.geometry().loc
    }
}
