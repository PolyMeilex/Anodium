use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use smithay::{
    backend::input,
    reexports::wayland_protocols::xdg_shell::server::xdg_toplevel::{self},
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor,
        seat::{GrabStartData, Seat},
        Serial,
    },
};

use crate::{
    desktop_layout::{Toplevel, Window, WindowList},
    shell::{MaximizeState, SurfaceData},
};

use super::{MoveResponse, Positioner};

#[derive(Debug)]
pub struct Tiling {
    geometry: Rectangle<i32, Logical>,
    pointer_position: Point<f64, Logical>,
    windows: Rc<RefCell<WindowList>>,
}

impl Tiling {
    #[allow(unused)]
    pub fn new(pointer_position: Point<f64, Logical>, geometry: Rectangle<i32, Logical>) -> Self {
        Self {
            geometry,
            pointer_position,
            windows: Default::default(),
        }
    }

    pub fn arange_windows(&mut self) {
        if self.windows.borrow().len() > 0 {
            let len = self.windows.borrow().len();
            let w = self.geometry.size.w / len.min(2) as i32;

            let mut loc = self.geometry.loc;

            for (id, window) in self.windows.borrow_mut().iter_mut().rev().enumerate() {
                if window.animation().is_exiting() {
                    continue;
                }

                let h = if id == 0 {
                    self.geometry.size.h
                } else {
                    self.geometry.size.h / (len - 1) as i32
                };

                window.toplevel().resize((w - 20, h - 20).into());
                window.set_location(
                    (
                        loc.x - window.geometry().loc.x + 10,
                        loc.y - window.geometry().loc.x + 10,
                    )
                        .into(),
                );

                if id < 1 {
                    loc.x += w;
                } else {
                    loc.y += h;
                }
            }
        }
    }
}

impl Positioner for Tiling {
    fn map_toplevel(&mut self, window: Window, mut reposition: bool) {
        if let Toplevel::Xdg(toplevel) = window.toplevel() {
            if let Some(state) = toplevel.current_state() {
                if state.states.contains(xdg_toplevel::State::Maximized)
                    || state.states.contains(xdg_toplevel::State::Fullscreen)
                {
                    reposition = false;
                }
            }
        }

        self.windows.borrow_mut().insert(window);

        if reposition {
            self.arange_windows();
        }
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

            let mut initial_window_location = window.location();

            // If surface is maximized then unmaximize it
            if let Toplevel::Xdg(ref surface) = toplevel {
                if let Some(current_state) = surface.current_state() {
                    if current_state.states.contains(xdg_toplevel::State::Maximized) {
                        let new_size = surface.get_surface().and_then(|surface| {
                            let fullscreen_state = compositor::with_states(&surface, |states| {
                                let mut data = states
                                    .data_map
                                    .get::<RefCell<SurfaceData>>()
                                    .unwrap()
                                    .borrow_mut();
                                let fullscreen_state = data.maximize_state;
                                data.maximize_state = MaximizeState::NotMaximized;

                                fullscreen_state
                            })
                            .unwrap();

                            if let MaximizeState::Current(data) = fullscreen_state {
                                Some(data.initial_size)
                            } else {
                                None
                            }
                        });

                        let fs_changed = surface.with_pending_state(|state| {
                            state.states.unset(xdg_toplevel::State::Maximized);
                            state.size = new_size;
                        });

                        if fs_changed.is_ok() {
                            surface.send_configure();

                            let pointer_pos = pointer.current_location();

                            if let (Some(current_size), Some(new_size)) = (current_state.size, new_size) {
                                let current_size = current_size.to_f64();
                                let window_location = initial_window_location.to_f64();
                                let pointer_win_pos = pointer_pos - window_location;

                                let p = pointer_win_pos.x / current_size.w;
                                let w = new_size.w as f64;

                                initial_window_location.x = (pointer_pos.x - w * p) as i32;
                            } else {
                                initial_window_location = pointer_pos.to_i32_round();
                            }
                        }
                    }
                }
            }

            Some(MoveResponse {
                initial_window_location,
                windows: self.windows.clone(),
            })
        } else {
            None
        }
    }

    fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {
        self.pointer_position = pos;
    }

    fn on_pointer_button(&mut self, button: input::MouseButton, state: input::ButtonState) {
        if let input::MouseButton::Left = button {
            if let input::ButtonState::Pressed = state {
                let windows = self.windows.borrow_mut();

                // TODO: other positioners should deactivate their windows too?
                for w in windows.iter() {
                    w.toplevel().set_activated(false);
                }

                let under = windows.surface_under(self.pointer_position);
                if let Some(under) = under {
                    if let Some(window) = windows.find(&under.0) {
                        window.toplevel().set_activated(true);
                    }
                }
            }
        };
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>) {
        self.geometry = geometry;
        self.arange_windows();
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
        {
            let mut windows = self.windows.borrow_mut();
            windows.refresh();
            windows.update_animations(delta);
        }

        // TODO: Optimize?
        self.arange_windows();
    }
}
