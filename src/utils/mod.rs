use std::fmt;

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
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

pub fn surface_bounding_box(surface: &WlSurface) -> Rectangle<i32, Logical> {
    let location: Point<i32, Logical> = Default::default();

    smithay::desktop::utils::bbox_from_surface_tree(surface, location)
}

/// Sends the frame callback to all the subsurfaces in this
/// surface that requested it
pub fn surface_send_frame(surface: &WlSurface, time: u32) {
    smithay::desktop::utils::send_frames_surface_tree(surface, time);
}
