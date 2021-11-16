use std::{cell::RefCell, rc::Rc};

use smithay::{
    reexports::wayland_server::protocol::{wl_pointer::ButtonState, wl_surface},
    utils::{Logical, Point},
    wayland::{
        seat::{AxisFrame, GrabStartData, PointerGrab, PointerInnerHandle},
        Serial,
    },
};

use crate::desktop_layout::{DesktopLayout, WindowSurface};

use super::{MoveAfterResizeState, SurfaceData};

pub struct MoveSurfaceGrab {
    pub start_data: GrabStartData,

    pub toplevel: WindowSurface,
    pub initial_window_location: Point<i32, Logical>,

    pub desktop_layout: Rc<RefCell<DesktopLayout>>,
}

impl PointerGrab for MoveSurfaceGrab {
    fn motion(
        &mut self,
        _handle: &mut PointerInnerHandle<'_>,
        location: Point<f64, Logical>,
        _focus: Option<(wl_surface::WlSurface, Point<i32, Logical>)>,
        _serial: Serial,
        _time: u32,
    ) {
        let delta = location - self.start_data.location;

        if let Some(window) = self.desktop_layout.borrow_mut().grabed_window.as_mut() {
            if let Some(surface) = window.toplevel().get_surface() {
                // Check if there is MoveAfterResize in progress
                let started = SurfaceData::with(surface, |data| {
                    matches!(
                        &data.move_after_resize_state,
                        // If done
                        MoveAfterResizeState::Current(_) |
                        // Or if non-existent
                        MoveAfterResizeState::None,
                    )
                });

                if started {
                    let new_location = self.initial_window_location.to_f64() + delta;
                    window.set_location(new_location.to_i32_round());
                }
            }
        }

        // TODO:
        //     if anodium.pointer_location.y < 5.0 {
        //         let mut initial_geometry = win.geometry();
        //         initial_geometry.loc += win.location();

        //         let target_geometry = anodium
        //             .output_map
        //             .borrow()
        //             .find_by_position(self.location.to_i32_round())
        //             .map(|o| o.geometry());

        //         if let Some(target_geometry) = target_geometry {
        //             anodium
        //                 .maximize_animation
        //                 .start(initial_geometry, target_geometry);
        //         }
        //     } else {
        //         anodium.maximize_animation.stop();
        //     }
    }

    fn button(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        button: u32,
        state: ButtonState,
        serial: Serial,
        time: u32,
    ) {
        handle.button(button, state, serial, time);
        if handle.current_pressed().is_empty() {
            // TODO:
            // if let Some(win) = self.windows.borrow_mut().find_mut(&self.toplevel) {
            // let pointer_location = self.pointer_position.to_i32_round();

            // if let Some(output) = anodium.output_map.borrow().find_by_position(pointer_location) {
            //     if pointer_location.y < 5 {
            //         win.maximize(output.geometry());
            //     }
            // }

            // anodium.maximize_animation.stop();
            // }

            {
                let desktop_layout = &mut *self.desktop_layout.borrow_mut();

                let window = desktop_layout.grabed_window.take().unwrap();

                let location = window.location() + window.geometry().loc;

                if let Some(key) = desktop_layout
                    .output_map
                    .find_by_position(location)
                    .map(|o| o.active_workspace())
                {
                    desktop_layout
                        .workspaces
                        .get_mut(key)
                        .unwrap()
                        .map_toplevel(window, false);
                } else {
                    desktop_layout
                        .active_workspace()
                        .map_toplevel(window, false);
                }
            }

            // No more buttons are pressed, release the grab.
            handle.unset_grab(serial, time);
        }
    }

    fn axis(&mut self, handle: &mut PointerInnerHandle<'_>, details: AxisFrame) {
        handle.axis(details)
    }

    fn start_data(&self) -> &GrabStartData {
        &self.start_data
    }
}
