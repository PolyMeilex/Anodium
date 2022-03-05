use smithay::{
    reexports::wayland_server::{
        protocol::{wl_pointer::ButtonState, wl_surface::WlSurface},
        DispatchData,
    },
    utils::{Logical, Point},
    wayland::{
        seat::{AxisFrame, PointerGrab, PointerGrabStartData, PointerInnerHandle},
        Serial,
    },
};

use {
    crate::State,
    anodium_framework::shell::{ShellEvent, ShellHandler},
    smithay::desktop,
};

impl ShellHandler for State {
    fn on_shell_event(&mut self, event: anodium_framework::shell::ShellEvent) {
        match event {
            ShellEvent::WindowCreated { window } => {
                self.space.map_window(&window, (0, 0), true);
                window.configure();
            }
            ShellEvent::WindowMove {
                toplevel,
                start_data,
                seat,
                serial,
            } => {
                let pointer = seat.get_pointer().unwrap();

                let window = self
                    .space
                    .window_for_surface(toplevel.get_surface().unwrap())
                    .cloned();

                if let Some(window) = window {
                    let initial_window_location = self.space.window_geometry(&window).unwrap().loc;

                    let grab = MoveSurfaceGrab {
                        start_data,
                        window,
                        initial_window_location,
                    };
                    pointer.set_grab(grab, serial, 0);
                }
            }
            ShellEvent::SurfaceCommit { surface } => {
                self.space.commit(&surface);
            }
            _ => {}
        }
    }

    fn window_location(
        &self,
        _window: &desktop::Window,
    ) -> smithay::utils::Point<i32, smithay::utils::Logical> {
        (0, 0).into()
    }
}

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData,

    pub window: desktop::Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab for MoveSurfaceGrab {
    fn motion(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        location: Point<f64, Logical>,
        _focus: Option<(WlSurface, Point<i32, Logical>)>,
        serial: Serial,
        time: u32,
        mut ddata: DispatchData,
    ) {
        handle.motion(location, None, serial, time);

        let state = ddata.get::<State>().unwrap();

        let delta = location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;

        state
            .space
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
