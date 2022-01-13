use std::cell::RefCell;

use smithay::{
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel,
        wayland_server::protocol::{wl_shell_surface, wl_surface::WlSurface},
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{compositor, Serial},
};

use crate::utils::LogResult;

#[derive(Default)]
pub struct SurfaceData {
    pub geometry: Option<Rectangle<i32, Logical>>,
    pub resize_state: ResizeState,
    pub move_after_resize_state: MoveAfterResizeState,
}

impl SurfaceData {
    #[allow(dead_code)]
    pub fn try_with<F, R>(surface: &WlSurface, cb: F) -> Option<R>
    where
        F: FnOnce(&SurfaceData) -> R,
    {
        compositor::with_states(surface, |states| {
            if let Some(data) = states.data_map.get::<RefCell<SurfaceData>>() {
                let data = data.borrow();
                Some(cb(&data))
            } else {
                warn!("Surface: {:?} does not have SurfaceData", surface);
                None
            }
        })
        .log_err("Surface is dead!")
        .ok()?
    }

    pub fn with<F, R>(surface: &WlSurface, cb: F) -> R
    where
        F: FnOnce(&SurfaceData) -> R,
    {
        compositor::with_states(surface, |states| {
            let data = states
                .data_map
                .get::<RefCell<SurfaceData>>()
                .expect("Surface does not have SurfaceData");

            let data = data.borrow();
            cb(&data)
        })
        .expect("The surface is dead")
    }

    pub fn with_mut<F, R>(surface: &WlSurface, cb: F) -> R
    where
        F: FnOnce(&mut SurfaceData) -> R,
    {
        compositor::with_states(surface, |states| {
            let data = states
                .data_map
                .get::<RefCell<SurfaceData>>()
                .expect("Surface does not have SurfaceData");

            let mut data = data.borrow_mut();
            cb(&mut data)
        })
        .expect("The surface is dead")
    }
}

/// Information about the fullscrean operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MoveAfterResizeData {
    /// The initial window location.
    pub initial_window_location: Point<i32, Logical>,
    /// The initial window geometry.
    pub initial_size: Size<i32, Logical>,

    /// The target window location.
    pub target_window_location: Point<i32, Logical>,
    /// The target window geometry.
    pub target_size: Size<i32, Logical>,
}

/// State of the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MoveAfterResizeState {
    /// Idle
    None,
    /// The surface was resized and moved
    Current(MoveAfterResizeData),
    /// The resize and move was requested, and the surface needs to ack the configure
    WaitingForAck(MoveAfterResizeData),
    /// Waiting for commit
    WaitingForCommit(MoveAfterResizeData),
}

impl Default for MoveAfterResizeState {
    fn default() -> Self {
        Self::None
    }
}

/// Information about the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ResizeData {
    /// The edges the surface is being resized with.
    pub edges: ResizeEdge,
    /// The initial window location.
    pub initial_window_location: Point<i32, Logical>,
    /// The initial window size (geometry width and height).
    pub initial_window_size: Size<i32, Logical>,
}

/// State of the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResizeState {
    /// The surface is not being resized.
    NotResizing,
    /// The surface is currently being resized.
    Resizing(ResizeData),
    /// The resize has finished, and the surface needs to ack the final configure.
    WaitingForFinalAck(ResizeData, Serial),
    /// The resize has finished, and the surface needs to commit its final state.
    WaitingForCommit(ResizeData),
}

impl Default for ResizeState {
    fn default() -> Self {
        ResizeState::NotResizing
    }
}

bitflags::bitflags! {
    pub struct ResizeEdge: u32 {
        const NONE = 0;
        const TOP = 1;
        const BOTTOM = 2;
        const LEFT = 4;
        const TOP_LEFT = 5;
        const BOTTOM_LEFT = 6;
        const RIGHT = 8;
        const TOP_RIGHT = 9;
        const BOTTOM_RIGHT = 10;
    }
}

impl From<wl_shell_surface::Resize> for ResizeEdge {
    #[inline]
    fn from(x: wl_shell_surface::Resize) -> Self {
        Self::from_bits(x.bits()).unwrap()
    }
}

impl From<ResizeEdge> for wl_shell_surface::Resize {
    #[inline]
    fn from(x: ResizeEdge) -> Self {
        Self::from_bits(x.bits()).unwrap()
    }
}

impl From<xdg_toplevel::ResizeEdge> for ResizeEdge {
    #[inline]
    fn from(x: xdg_toplevel::ResizeEdge) -> Self {
        Self::from_bits(x.to_raw()).unwrap()
    }
}

impl From<ResizeEdge> for xdg_toplevel::ResizeEdge {
    #[inline]
    fn from(x: ResizeEdge) -> Self {
        Self::from_raw(x.bits()).unwrap()
    }
}
