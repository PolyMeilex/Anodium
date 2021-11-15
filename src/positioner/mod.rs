use smithay::{
    backend::input::{ButtonState, MouseButton},
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel::ResizeEdge,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        seat::{GrabStartData, Seat},
        Serial,
    },
};

use crate::desktop_layout::{WindowSurface, Window};

pub mod floating;
pub mod tiling;
pub mod universal;

#[allow(unused)]
pub trait Positioner: std::fmt::Debug {
    fn map_toplevel(&mut self, window: Window, reposition: bool);
    fn unmap_toplevel(&mut self, toplevel: &WindowSurface) -> Option<Window>;

    fn move_request(
        &mut self,
        toplevel: &WindowSurface,
        seat: &Seat,
        serial: Serial,
        start_data: &GrabStartData,
    ) -> Option<MoveResponse>;

    fn resize_request(
        &mut self,
        toplevel: &WindowSurface,
        seat: &Seat,
        serial: Serial,
        start_data: GrabStartData,
        edges: ResizeEdge,
    ) {
    }

    fn maximize_request(&mut self, toplevel: &WindowSurface) {}
    fn unmaximize_request(&mut self, toplevel: &WindowSurface) {}

    fn with_windows_rev(&self, cb: &mut dyn FnMut(&Window));

    fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        None
    }

    fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        None
    }
    fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut Window> {
        None
    }

    fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {}
    fn on_pointer_button(&mut self, button: MouseButton, state: ButtonState) {}

    fn set_geometry(&mut self, size: Rectangle<i32, Logical>) {}
    fn geometry(&self) -> Rectangle<i32, Logical>;

    fn send_frames(&self, time: u32) {}
    fn update(&mut self, delta: f64) {}
}

#[derive(Debug)]
pub struct MoveResponse {
    pub initial_window_location: Point<i32, Logical>,
}
