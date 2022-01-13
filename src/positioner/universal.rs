use smithay::{
    desktop::Kind,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
};

use super::{floating::Floating, tiling::Tiling, MoveResponse, Positioner};

use crate::framework::surface_data::ResizeEdge;
use crate::window::Window;

#[allow(unused)]
#[derive(Debug)]
pub enum PositionerMode {
    Floating,
    Tiling,
}

#[derive(Debug)]
pub struct Universal {
    floating: Floating,
    tiling: Tiling,

    mode: PositionerMode,
}

impl Universal {
    pub fn new(pointer_position: Point<f64, Logical>, geometry: Rectangle<i32, Logical>) -> Self {
        Self {
            floating: Floating::new(pointer_position, geometry),
            tiling: Tiling::new(pointer_position, geometry),
            mode: PositionerMode::Floating,
        }
    }
}

impl Positioner for Universal {
    fn map_toplevel(&mut self, window: Window, reposition: bool) {
        match self.mode {
            PositionerMode::Floating => self.floating.map_toplevel(window, reposition),
            PositionerMode::Tiling => self.tiling.map_toplevel(window, reposition),
        }
    }

    fn unmap_toplevel(&mut self, toplevel: &Kind) -> Option<Window> {
        if let Some(win) = self.floating.unmap_toplevel(toplevel) {
            Some(win)
        } else {
            self.tiling.unmap_toplevel(toplevel)
        }
    }

    fn move_request(
        &mut self,
        toplevel: &Kind,
        seat: &smithay::wayland::seat::Seat,
        serial: smithay::wayland::Serial,
        start_data: &smithay::wayland::seat::GrabStartData,
    ) -> Option<MoveResponse> {
        if let Some(req) = self
            .floating
            .move_request(toplevel, seat, serial, start_data)
        {
            Some(req)
        } else {
            self.tiling.move_request(toplevel, seat, serial, start_data)
        }
    }

    fn resize_request(
        &mut self,
        toplevel: &Kind,
        seat: &smithay::wayland::seat::Seat,
        serial: smithay::wayland::Serial,
        start_data: smithay::wayland::seat::GrabStartData,
        edges: ResizeEdge,
    ) {
        self.floating
            .resize_request(toplevel, seat, serial, start_data.clone(), edges);
        self.tiling
            .resize_request(toplevel, seat, serial, start_data, edges);
    }

    fn maximize_request(&mut self, toplevel: &Kind) {
        self.floating.maximize_request(toplevel);
        self.tiling.maximize_request(toplevel);
    }

    fn unmaximize_request(&mut self, toplevel: &Kind) {
        self.floating.unmaximize_request(toplevel);
        self.tiling.unmaximize_request(toplevel);
    }

    fn with_windows_rev(&self, cb: &mut dyn FnMut(&Window)) {
        self.tiling.with_windows_rev(cb);
        self.floating.with_windows_rev(cb);
    }

    fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let fr = self.floating.surface_under(point);

        if fr.is_some() {
            fr
        } else {
            self.tiling.surface_under(point)
        }
    }

    fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        let fr = self.floating.find_window(surface);

        if fr.is_some() {
            fr
        } else {
            self.tiling.find_window(surface)
        }
    }

    fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut Window> {
        let fr = self.floating.find_window_mut(surface);

        if fr.is_some() {
            fr
        } else {
            self.tiling.find_window_mut(surface)
        }
    }

    fn on_pointer_move(&mut self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) {
        self.floating.on_pointer_move(pos);
        self.tiling.on_pointer_move(pos);
    }

    fn on_pointer_button(
        &mut self,
        button: smithay::backend::input::MouseButton,
        state: smithay::backend::input::ButtonState,
    ) {
        self.floating.on_pointer_button(button, state);
        self.tiling.on_pointer_button(button, state);
    }

    fn set_geometry(&mut self, size: smithay::utils::Rectangle<i32, smithay::utils::Logical>) {
        self.floating.set_geometry(size);
        self.tiling.set_geometry(size);
    }

    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.floating.geometry()
    }

    fn send_frames(&self, time: u32) {
        self.floating.send_frames(time);
        self.tiling.send_frames(time);
    }

    fn update(&mut self, delta: f64) {
        self.floating.update(delta);
        self.tiling.update(delta);
    }
}
