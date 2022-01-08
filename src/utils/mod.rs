use std::{cell::RefCell, fmt};

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{self, SubsurfaceCachedState, TraversalAction},
        shell::xdg::{self, ToplevelSurface},
    },
};

use crate::{framework::surface_data::SurfaceData, window::WindowSurface};

pub mod imgui_input;
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
    let mut bounding_box = Rectangle::from_loc_and_size((0, 0), (0, 0));
    let location: Point<i32, Logical> = Default::default();

    compositor::with_surface_tree_downward(
        surface,
        location,
        |_, states, &loc| {
            let mut loc = loc;

            let data = states.data_map.get::<RefCell<SurfaceData>>();
            let size = data.and_then(|d| d.borrow().size());

            if let Some(size) = size {
                if states.role == Some("subsurface") {
                    let current = states.cached_state.current::<SubsurfaceCachedState>();
                    loc += current.location;
                }

                // Update the bounding box.
                bounding_box = bounding_box.merge(Rectangle::from_loc_and_size(loc, size));

                TraversalAction::DoChildren(loc)
            } else {
                // If the parent surface is unmapped, then the child surfaces are hidden as
                // well, no need to consider them here.
                TraversalAction::SkipChildren
            }
        },
        |_, _, _| {},
        |_, _, _| true,
    );

    bounding_box
}

/// Sends the frame callback to all the subsurfaces in this
/// surface that requested it
pub fn surface_send_frame(surface: &WlSurface, time: u32) {
    compositor::with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_, states, &()| {
            // the surface may not have any user_data if it is a subsurface and has not
            // yet been commited
            SurfaceData::send_frame(&mut *states.cached_state.current(), time)
        },
        |_, _, &()| true,
    );
}
