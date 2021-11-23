use smithay::{
    reexports::wayland_server::{
        protocol::{wl_pointer::ButtonState, wl_surface},
        DispatchData,
    },
    utils::{Logical, Point},
    wayland::{
        seat::{AxisFrame, GrabStartData, PointerGrab, PointerInnerHandle},
        Serial,
    },
};

use crate::popup::PopupSurface;

pub struct PopupGrab {
    pub start_data: GrabStartData,
    pub popup: PopupSurface,
}

// TODO
impl PointerGrab for PopupGrab {
    fn motion(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        location: Point<f64, Logical>,
        focus: Option<(wl_surface::WlSurface, Point<i32, Logical>)>,
        serial: Serial,
        time: u32,
        _ddata: DispatchData,
    ) {
        // let anodium = ddata.get::<Anodium>().unwrap();
        handle.motion(location, focus, serial, time);
        handle.unset_grab(serial, time);
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
        // let anodium = ddata.get::<Anodium>().unwrap();
        handle.button(button, state, serial, time);

        // if !handle.current_pressed().is_empty() {
        // No more buttons are pressed, release the grab.
        handle.unset_grab(serial, time);
        // self.popup.dismiss();
        // }
    }

    fn axis(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        details: AxisFrame,
        _ddata: DispatchData,
    ) {
        handle.axis(details);
    }

    fn start_data(&self) -> &GrabStartData {
        &self.start_data
    }
}
