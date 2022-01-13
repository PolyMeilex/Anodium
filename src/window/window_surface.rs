use smithay::{
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel, wayland_server::protocol::wl_surface,
    },
    utils::{Logical, Point, Size},
    wayland::shell::xdg::ToplevelSurface,
};

#[cfg(feature = "xwayland")]
use crate::framework::shell::X11Surface;

#[derive(Clone, Debug, PartialEq)]
pub enum WindowSurface {
    Xdg(ToplevelSurface),
    #[cfg(feature = "xwayland")]
    X11(X11Surface),
}

impl WindowSurface {
    pub fn alive(&self) -> bool {
        match *self {
            WindowSurface::Xdg(ref t) => t.alive(),
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(ref t) => t.alive(),
        }
    }

    pub fn get_surface(&self) -> Option<&wl_surface::WlSurface> {
        match *self {
            WindowSurface::Xdg(ref t) => t.get_surface(),
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(ref t) => t.get_surface(),
        }
    }

    pub fn maximize(&self, size: Size<i32, Logical>) {
        if let WindowSurface::Xdg(ref t) = self {
            let res = t.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Maximized);
                state.size = Some(size);
            });
            if res.is_ok() {
                t.send_configure();
            }
        }
    }

    pub fn unmaximize(&self, size: Option<Size<i32, Logical>>) {
        if let WindowSurface::Xdg(ref t) = self {
            let ret = t.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Maximized);
                state.size = size;
            });
            if ret.is_ok() {
                t.send_configure();
            }
        }
    }

    pub fn resize(&self, size: Size<i32, Logical>) {
        match self {
            WindowSurface::Xdg(t) => {
                let res = t.with_pending_state(|state| {
                    state.size = Some(size);
                });
                if res.is_ok() {
                    t.send_configure();
                }
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(t) => t.resize(size.w as u32, size.h as u32),
        };
    }

    pub fn notify_move(&self, _pos: Point<i32, Logical>) {
        #[cfg(feature = "xwayland")]
        if let WindowSurface::X11(t) = self {
            t.move_to(pos.x as i32, pos.y as i32)
        }
    }
}
