use smithay::{
    backend::input,
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        seat::{GrabStartData, Seat},
        Serial,
    },
};

use crate::{
    framework::surface_data::{
        MoveAfterResizeData, MoveAfterResizeState, ResizeData, ResizeEdge, ResizeState, SurfaceData,
    },
    grabs::ResizeSurfaceGrab,
    window::{Window, WindowList, WindowSurface},
};

use super::{MoveResponse, Positioner};

#[derive(Debug)]
pub struct Floating {
    geometry: Rectangle<i32, Logical>,
    pointer_position: Point<f64, Logical>,
    windows: WindowList,
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
        if let WindowSurface::Xdg(toplevel) = window.toplevel() {
            if let Some(state) = toplevel.current_state() {
                if state.states.contains(xdg_toplevel::State::Maximized)
                    || state.states.contains(xdg_toplevel::State::Fullscreen)
                {
                    reposition = false;
                }
            }
        } else if let WindowSurface::X11(_) = window.toplevel() {
            reposition = false;
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

        self.windows.insert(window);
    }

    fn unmap_toplevel(&mut self, toplevel: &WindowSurface) -> Option<Window> {
        self.windows.remove(toplevel)
    }

    fn move_request(
        &mut self,
        toplevel: &WindowSurface,
        seat: &Seat,
        _serial: Serial,
        _start_data: &GrabStartData,
    ) -> Option<MoveResponse> {
        if let Some(window) = self.windows.find(toplevel) {
            let pointer = seat.get_pointer().unwrap();

            let mut target_window_location = window.location();

            // If surface is maximized then unmaximize it
            if let WindowSurface::Xdg(ref surface) = toplevel {
                if let Some(current_state) = surface.current_state() {
                    if current_state
                        .states
                        .contains(xdg_toplevel::State::Maximized)
                    {
                        let new_size = surface.get_surface().and_then(|surface| {
                            SurfaceData::with_mut(surface, |data| {
                                let fullscreen_state = data.move_after_resize_state;
                                data.move_after_resize_state = MoveAfterResizeState::None;

                                if let MoveAfterResizeState::Current(rdata) = fullscreen_state {
                                    Some(rdata.initial_size)
                                } else {
                                    None
                                }
                            })
                        });

                        let fs_changed = surface.with_pending_state(|state| {
                            state.states.unset(xdg_toplevel::State::Maximized);
                            state.size = new_size;
                        });

                        if fs_changed.is_ok() {
                            surface.send_configure();

                            let pointer_pos = pointer.current_location();

                            if let (Some(initial_size), Some(target_size)) =
                                (current_state.size, new_size)
                            {
                                let initial_window_location = target_window_location;
                                let pointer_win_pos =
                                    pointer_pos - initial_window_location.to_f64();

                                let p = pointer_win_pos.x / initial_size.w as f64;
                                let w = target_size.w as f64;

                                target_window_location.x = (pointer_pos.x - w * p) as i32;

                                if let Some(surface) = surface.get_surface() {
                                    SurfaceData::with_mut(surface, |data| {
                                        data.move_after_resize_state =
                                            MoveAfterResizeState::WaitingForAck(
                                                MoveAfterResizeData {
                                                    initial_window_location,
                                                    initial_size,

                                                    target_window_location,
                                                    target_size,
                                                },
                                            );
                                    });
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
            })
        } else {
            None
        }
    }

    fn resize_request(
        &mut self,
        toplevel: &WindowSurface,
        seat: &Seat,
        serial: Serial,
        start_data: GrabStartData,
        edges: ResizeEdge,
    ) {
        if let Some(window) = self.windows.find(toplevel) {
            let initial_window_location = window.location();
            let initial_window_size = window.geometry().size;

            SurfaceData::with_mut(toplevel.get_surface().unwrap(), |data| {
                data.resize_state = ResizeState::Resizing(ResizeData {
                    edges,
                    initial_window_location,
                    initial_window_size,
                });
            });

            let grab = ResizeSurfaceGrab {
                start_data,
                toplevel: toplevel.clone(),
                edges,
                initial_window_size,
                last_window_size: initial_window_size,
            };

            let pointer = seat.get_pointer().unwrap();
            pointer.set_grab(grab, serial);
        };
    }

    fn maximize_request(&mut self, toplevle: &WindowSurface) {
        if let Some(window) = self.windows.find_mut(toplevle) {
            window.maximize(self.geometry);
        }
    }

    fn unmaximize_request(&mut self, toplevle: &WindowSurface) {
        if let Some(window) = self.windows.find_mut(toplevle) {
            window.unmaximize();
        }
    }

    fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {
        self.pointer_position = pos;
    }

    fn on_pointer_button(&mut self, button: input::MouseButton, state: input::ButtonState) {
        if let input::MouseButton::Left = button {
            if let input::ButtonState::Pressed = state {
                self.windows
                    .get_surface_and_bring_to_top(self.pointer_position);
            }
        };
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>) {
        self.geometry = geometry;
    }

    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.geometry
    }

    fn with_windows_rev(&self, cb: &mut dyn FnMut(&Window)) {
        for w in self.windows.iter().rev() {
            cb(w)
        }
    }

    fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        self.windows.surface_under(point)
    }

    fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        self.windows.find(surface)
    }

    fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut Window> {
        self.windows.find_mut(surface)
    }

    fn send_frames(&self, time: u32) {
        self.windows.send_frames(time);
    }

    fn update(&mut self, delta: f64) {
        self.windows.refresh();
        self.windows.update_animations(delta);
    }
}
