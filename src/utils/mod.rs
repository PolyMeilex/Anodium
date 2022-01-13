use std::fmt;

use smithay::{
    desktop::{Kind, PopupKind},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::shell::xdg::{self, ToplevelSurface},
};

use crate::window::WindowSurface;

mod iterators;

pub use iterators::{VisibleWorkspaceIter, VisibleWorkspaceIterMut};

pub trait AsWlSurface {
    fn as_surface(&self) -> Option<&WlSurface>;
}

impl AsWlSurface for WlSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        Some(self)
    }
}

impl AsWlSurface for WindowSurface {
    fn as_surface(&self) -> Option<&WlSurface> {
        self.get_surface()
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

pub trait LogResult {
    /// Log if error,
    /// do nothing otherwhise
    fn log_err(self, label: &str) -> Self;
}

impl<D, E: fmt::Debug> LogResult for Result<D, E> {
    fn log_err(self, label: &str) -> Self {
        if let Err(ref err) = self {
            error!("{} {:?}", label, err);
        }

        self
    }
}
