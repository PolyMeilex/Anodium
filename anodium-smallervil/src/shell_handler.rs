use crate::State;
use anodium_framework::shell::{ShellHandler, X11WindowUserData};

use smithay::{
    desktop,
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

impl ShellHandler for State {
    fn window_created(&mut self, window: desktop::Window) {
        self.space.map_window(&window, (0, 0), true);
        window.configure();
    }

    fn window_move(
        &mut self,
        window: desktop::Window,
        start_data: PointerGrabStartData,
        seat: smithay::wayland::seat::Seat,
        serial: Serial,
    ) {
        let pointer = seat.get_pointer().unwrap();

        let initial_window_location = self.space.window_location(&window).unwrap();

        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };
        pointer.set_grab(grab, serial, 0);
    }

    fn surface_commit(&mut self, surface: WlSurface) {
        self.space.commit(&surface);
    }

    fn window_resize(
        &mut self,
        window: desktop::Window,
        start_data: PointerGrabStartData,
        seat: smithay::wayland::seat::Seat,
        edges: ResizeEdge,
        serial: Serial,
    ) {
        let pointer = seat.get_pointer().unwrap();

        let wl_surface = window.toplevel().get_surface();

        if let Some(wl_surface) = wl_surface {
            let window_location = self.space.window_location(&window).unwrap();
            let window_size = window.geometry().size;

            SurfaceData::with_mut(wl_surface, |data| {
                data.resize_state
                    .start_resize(edges, window_location, window_size);
            });

            let grab = ResizeSurfaceGrab::new(start_data, window, edges, window_size);

            pointer.set_grab(grab, serial, 0);
        }
    }

    fn window_got_resized(
        &mut self,
        window: desktop::Window,
        new_location_x: Option<i32>,
        new_location_y: Option<i32>,
    ) {
        let mut new_location = self.space.window_location(&window).unwrap_or_default();

        if let Some(x) = new_location_x {
            new_location.x = x;
        }

        if let Some(y) = new_location_y {
            new_location.y = y;
        }

        self.space.map_window(&window, new_location, false);
    }

    fn xwayland_configure_request(
        &mut self,
        _conn: std::sync::Arc<smithay::reexports::x11rb::rust_connection::RustConnection>,
        event: smithay::reexports::x11rb::protocol::xproto::ConfigureRequestEvent,
    ) {
        let window = self
            .space
            .windows()
            .find(|win| {
                win.user_data()
                    .get::<X11WindowUserData>()
                    .map(|win| win.window == event.window)
                    .unwrap_or(false)
            })
            .cloned();

        if let Some(window) = window {
            self.space
                .map_window(&window, (event.x as i32, event.y as i32), false);
        }
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

use smithay::{
    desktop::Kind,
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel, wayland_server::protocol::wl_surface,
    },
    utils::Size,
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};

use anodium_framework::surface_data::{ResizeEdge, ResizeState, SurfaceData};

pub struct ResizeSurfaceGrab {
    pub start_data: PointerGrabStartData,
    pub window: desktop::Window,
    pub edges: ResizeEdge,
    pub initial_window_size: Size<i32, Logical>,
    pub last_window_size: Size<i32, Logical>,
}

impl ResizeSurfaceGrab {
    fn new(
        start_data: PointerGrabStartData,
        window: desktop::Window,
        edges: ResizeEdge,
        window_size: Size<i32, Logical>,
    ) -> Self {
        Self {
            start_data,
            window,
            edges,
            initial_window_size: window_size,
            last_window_size: window_size,
        }
    }
}

impl PointerGrab for ResizeSurfaceGrab {
    fn motion(
        &mut self,
        handle: &mut PointerInnerHandle<'_>,
        location: Point<f64, Logical>,
        _focus: Option<(wl_surface::WlSurface, Point<i32, Logical>)>,
        serial: Serial,
        time: u32,
        _ddata: DispatchData,
    ) {
        handle.motion(location, None, serial, time);

        let (mut dx, mut dy) = (location - self.start_data.location).into();

        let mut new_window_width = self.initial_window_size.w;
        let mut new_window_height = self.initial_window_size.h;

        let left_right = ResizeEdge::LEFT | ResizeEdge::RIGHT;
        let top_bottom = ResizeEdge::TOP | ResizeEdge::BOTTOM;

        if self.edges.intersects(left_right) {
            if self.edges.intersects(ResizeEdge::LEFT) {
                dx = -dx;
            }

            new_window_width = (self.initial_window_size.w as f64 + dx) as i32;
        }

        if self.edges.intersects(top_bottom) {
            if self.edges.intersects(ResizeEdge::TOP) {
                dy = -dy;
            }

            new_window_height = (self.initial_window_size.h as f64 + dy) as i32;
        }

        let (min_size, max_size) =
            with_states(self.window.toplevel().get_surface().unwrap(), |states| {
                let data = states.cached_state.current::<SurfaceCachedState>();
                (data.min_size, data.max_size)
            })
            .expect("Can't resize surface");

        let min_width = min_size.w.max(1);
        let min_height = min_size.h.max(1);
        let max_width = if max_size.w == 0 {
            i32::max_value()
        } else {
            max_size.w
        };
        let max_height = if max_size.h == 0 {
            i32::max_value()
        } else {
            max_size.h
        };

        new_window_width = new_window_width.max(min_width).min(max_width);
        new_window_height = new_window_height.max(min_height).min(max_height);

        self.last_window_size = (new_window_width, new_window_height).into();

        match self.window.toplevel() {
            Kind::Xdg(xdg) => {
                let ret = xdg.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Resizing);
                    state.size = Some(self.last_window_size);
                });
                if ret.is_ok() {
                    xdg.send_configure();
                }
            }
            #[cfg(feature = "xwayland")]
            Kind::X11(_) => {
                // TODO: What to do here? Send the update via X11?
            }
        }
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

            if let Kind::Xdg(xdg) = self.window.toplevel() {
                let ret = xdg.with_pending_state(|state| {
                    state.states.unset(xdg_toplevel::State::Resizing);
                    state.size = Some(self.last_window_size);
                });
                if ret.is_ok() {
                    xdg.send_configure();
                }
            }

            SurfaceData::with_mut(self.window.toplevel().get_surface().unwrap(), |data| {
                if let ResizeState::Resizing(resize_data) = data.resize_state {
                    data.resize_state = ResizeState::WaitingForCommit(resize_data);
                } else {
                    panic!("invalid resize state: {:?}", data.resize_state);
                }
            });
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
