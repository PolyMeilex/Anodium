use smithay::{
    backend::{
        input::{ButtonState, MouseButton},
        SwapBuffersError,
    },
    desktop::Kind,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        seat::{GrabStartData, Seat},
        Serial,
    },
};

use crate::window::Window;
use crate::{framework::surface_data::ResizeEdge, render::renderer::RenderFrame};

pub mod floating;
pub mod tiling;
pub mod universal;

#[allow(unused)]
pub trait Positioner: std::fmt::Debug {
    fn map_toplevel(&mut self, window: Window, reposition: bool);
    fn unmap_toplevel(&mut self, toplevel: &Kind) -> Option<Window>;

    fn move_request(
        &mut self,
        toplevel: &Kind,
        seat: &Seat,
        serial: Serial,
        start_data: &GrabStartData,
    ) -> Option<MoveResponse>;

    fn resize_request(
        &mut self,
        toplevel: &Kind,
        seat: &Seat,
        serial: Serial,
        start_data: GrabStartData,
        edges: ResizeEdge,
    ) {
    }

    fn maximize_request(&mut self, toplevel: &Kind) {}
    fn unmaximize_request(&mut self, toplevel: &Kind) {}

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

    fn draw_windows(
        &self,
        frame: &mut RenderFrame,
        output_rect: Rectangle<i32, Logical>,
        output_scale: f64,
    ) -> Result<(), SwapBuffersError> {
        let mut render = move |window: &Window| {
            // skip windows that do not overlap with a given output
            if !output_rect.overlaps(window.bbox_in_comp_space()) {
                return;
            }

            window.render(frame, output_rect.loc, output_scale);
        };

        self.with_windows_rev(&mut render);

        Ok(())
    }
}

#[derive(Debug)]
pub struct MoveResponse {
    pub initial_window_location: Point<i32, Logical>,
}
