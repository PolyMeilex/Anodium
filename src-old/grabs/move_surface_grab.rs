use smithay::{
    desktop::{self},
    reexports::wayland_server::{
        protocol::{wl_pointer::ButtonState, wl_surface},
        DispatchData,
    },
    utils::{Logical, Point},
    wayland::{
        seat::{AxisFrame, PointerGrab, PointerGrabStartData, PointerInnerHandle},
        Serial,
    },
};

use crate::state::Anodium;

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData,

    pub window: desktop::Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab for MoveSurfaceGrab {
    fn motion(
        &mut self,
        _handle: &mut PointerInnerHandle<'_>,
        location: Point<f64, Logical>,
        _focus: Option<(wl_surface::WlSurface, Point<i32, Logical>)>,
        _serial: Serial,
        _time: u32,
        mut ddata: DispatchData,
    ) {
        let anodium = ddata.get::<Anodium>().unwrap();

        let delta = location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;

        anodium
            .region_manager
            .find_window_workspace(&self.window)
            .unwrap()
            .space_mut()
            .map_window(&self.window, new_location.to_i32_round(), false);
    }

    fn button(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        button: u32,
        state: ButtonState,
        serial: Serial,
        time: u32,
        _ddata: DispatchData,
    ) {
        handle.button(button, state, serial, time);
        if handle.current_pressed().is_empty() {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(serial, time);
        }
    }

    fn axis(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        details: AxisFrame,
        _ddata: DispatchData,
    ) {
        handle.axis(details)
    }

    fn start_data(&self) -> &PointerGrabStartData {
        &self.start_data
    }
}
