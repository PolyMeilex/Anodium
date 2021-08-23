use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use smithay::{
    backend::input,
    reexports::wayland_protocols::xdg_shell::server::xdg_toplevel::{self, ResizeEdge},
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor,
        seat::{GrabStartData, Seat},
        Serial,
    },
};

use crate::{
    desktop_layout::{Toplevel, Window, WindowList},
    shell::{
        resize_surface_grab::ResizeSurfaceGrab,
        surface_data::{ResizeData, ResizeState},
        MoveAfterResizeData, MoveAfterResizeState, SurfaceData,
    },
};

use super::{MoveResponse, Positioner};

#[derive(Debug)]
pub struct Floating {
    geometry: Rectangle<i32, Logical>,
    pointer_position: Point<f64, Logical>,
    windows: Rc<RefCell<WindowList>>,
}

impl Floating {
    pub fn new(pointer_position: Point<f64, Logical>, geometry: Rectangle<i32, Logical>) -> Self {
        Self {
            geometry,
            pointer_position,
            windows: Default::default(),
        }
    }
}

impl Positioner for Floating {
    fn map_toplevel(&mut self, mut window: Window, mut reposition: bool) {
        if let Toplevel::Xdg(toplevel) = window.toplevel() {
            if let Some(state) = toplevel.current_state() {
                if state.states.contains(xdg_toplevel::State::Maximized)
                    || state.states.contains(xdg_toplevel::State::Fullscreen)
                {
                    reposition = false;
                }
            }
        }

        if reposition {
            let geometry = window.geometry();
            // |==================|
            // |=====|=====|======|
            // |==================|
            let x = self.geometry.loc.x + (self.geometry.size.w - geometry.size.w) / 2;
            let y = self.geometry.loc.y + (self.geometry.size.h - geometry.size.h) / 2;
            window.set_location((x, y).into());
        }

        self.windows.borrow_mut().insert(window);
    }

    fn unmap_toplevel(&mut self, toplevel: &Toplevel) -> Option<Window> {
        self.windows.borrow_mut().remove(toplevel)
    }

    fn move_request(
        &mut self,
        toplevel: &Toplevel,
        seat: &Seat,
        _serial: Serial,
        _start_data: &GrabStartData,
    ) -> Option<MoveResponse> {
        if let Some(window) = self.windows.borrow().find(toplevel) {
            let pointer = seat.get_pointer().unwrap();

            let mut target_window_location = window.location();

            // If surface is maximized then unmaximize it
            if let Toplevel::Xdg(ref surface) = toplevel {
                if let Some(current_state) = surface.current_state() {
                    if current_state.states.contains(xdg_toplevel::State::Maximized) {
                        let new_size = surface.get_surface().and_then(|surface| {
                            compositor::with_states(&surface, |states| {
                                let mut data = states
                                    .data_map
                                    .get::<RefCell<SurfaceData>>()
                                    .unwrap()
                                    .borrow_mut();
                                let fullscreen_state = data.move_after_resize_state;
                                data.move_after_resize_state = MoveAfterResizeState::None;

                                if let MoveAfterResizeState::Current(rdata) = fullscreen_state {
                                    Some(rdata.initial_size)
                                } else {
                                    None
                                }
                            })
                            .unwrap()
                        });

                        let fs_changed = surface.with_pending_state(|state| {
                            state.states.unset(xdg_toplevel::State::Maximized);
                            state.size = new_size;
                        });

                        if fs_changed.is_ok() {
                            surface.send_configure();

                            let pointer_pos = pointer.current_location();

                            if let (Some(initial_size), Some(target_size)) = (current_state.size, new_size) {
                                let initial_window_location = target_window_location;
                                let pointer_win_pos = pointer_pos - initial_window_location.to_f64();

                                let p = pointer_win_pos.x / initial_size.w as f64;
                                let w = target_size.w as f64;

                                target_window_location.x = (pointer_pos.x - w * p) as i32;

                                if let Some(surface) = surface.get_surface() {
                                    compositor::with_states(&surface, |states| {
                                        let mut data = states
                                            .data_map
                                            .get::<RefCell<SurfaceData>>()
                                            .unwrap()
                                            .borrow_mut();

                                        data.move_after_resize_state =
                                            MoveAfterResizeState::WaitingForAck(MoveAfterResizeData {
                                                initial_window_location,
                                                initial_size,

                                                target_window_location,
                                                target_size,
                                            });
                                    })
                                    .unwrap();
                                } else {
                                    target_window_location = pointer_pos.to_i32_round();
                                }
                            }
                        }
                    }
                }
            }

            Some(MoveResponse {
                initial_window_location: target_window_location,
                windows: self.windows.clone(),
            })
        } else {
            None
        }
    }

    fn resize_request(
        &mut self,
        toplevel: &Toplevel,
        seat: &Seat,
        serial: Serial,
        start_data: GrabStartData,
        edges: ResizeEdge,
    ) {
        if let Some(window) = self.windows.borrow().find(toplevel) {
            let initial_window_location = window.location();
            let initial_window_size = window.geometry().size;

            compositor::with_states(toplevel.get_surface().unwrap(), move |states| {
                states
                    .data_map
                    .get::<RefCell<SurfaceData>>()
                    .unwrap()
                    .borrow_mut()
                    .resize_state = ResizeState::Resizing(ResizeData {
                    edges: edges.into(),
                    initial_window_location,
                    initial_window_size,
                });
            })
            .unwrap();

            let grab = ResizeSurfaceGrab {
                start_data,
                toplevel: toplevel.clone(),
                edges: edges.into(),
                initial_window_size,
                last_window_size: initial_window_size,
            };

            let pointer = seat.get_pointer().unwrap();
            pointer.set_grab(grab, serial);
        };
    }

    fn maximize_request(&mut self, toplevle: &Toplevel) {
        if let Some(window) = self.windows.borrow_mut().find_mut(toplevle) {
            window.maximize(self.geometry);
        }
    }

    fn unmaximize_request(&mut self, toplevle: &Toplevel) {
        if let Some(window) = self.windows.borrow_mut().find_mut(toplevle) {
            window.unmaximize();
        }
    }

    fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {
        self.pointer_position = pos;
    }

    fn on_pointer_button(&mut self, button: input::MouseButton, state: input::ButtonState) {
        if let input::MouseButton::Left = button {
            if let input::ButtonState::Pressed = state {
                let mut windows = self.windows.borrow_mut();
                let under = windows.surface_under(self.pointer_position);
                if let Some(under) = under {
                    windows.bring_surface_to_top(&under.0);
                }
            }
        };
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>) {
        self.geometry = geometry;
    }

    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.geometry
    }

    fn windows(&self) -> Ref<WindowList> {
        self.windows.borrow()
    }

    fn windows_mut(&self) -> RefMut<WindowList> {
        self.windows.borrow_mut()
    }

    fn send_frames(&self, time: u32) {
        self.windows.borrow().send_frames(time);
    }

    fn update(&mut self, delta: f64) {
        let mut windows = self.windows.borrow_mut();
        windows.refresh();
        windows.update_animations(delta);
    }
}
