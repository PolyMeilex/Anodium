use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    delegate_xdg_shell,
    desktop::{
        Kind, PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window,
        WindowSurfaceType,
    },
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Rectangle, Serial},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use crate::{
    data::seat::SeatState,
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    State,
};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface().clone();

        let window = Window::new(Kind::Xdg(surface));
        self.space.map_window(&window, (0, 0), None, false);

        fn on_initial_commit(state: &mut State, window: Window, surface: &WlSurface) {
            // Send initial configure
            window.configure();

            /// Called once first buffer is attached
            fn on_window_mapped(state: &mut State, window: Window) {
                let pointer_pos = SeatState::for_seat(&state.seat).pointer_pos();

                let loc = state.space.output_under(pointer_pos).next().map(|output| {
                    let output = state.space.output_geometry(output).unwrap();
                    let window = window.geometry();

                    let x = output.size.w / 2 - window.size.w / 2;
                    let y = output.size.h / 2 - window.size.h / 2;

                    (output.loc.x + x, output.loc.y + y)
                });

                if let Some(loc) = loc {
                    state.space.map_window(&window, loc, None, false);
                }
            }

            fn on_commit(state: &mut State, window: Window, surface: &WlSurface) {
                let buffer_attached =
                    with_renderer_surface_state(surface, |data| data.wl_buffer().is_some());

                if buffer_attached {
                    // Window got mapped so we can position it
                    on_window_mapped(state, window);
                } else {
                    // Wait for nex commit
                    state
                        .commit_dispatcher
                        .on_next_commit(surface.clone(), move |state, surface| {
                            on_commit(state, window, surface)
                        });
                }
            }

            on_commit(state, window, surface);
        }

        self.commit_dispatcher
            .on_next_commit(wl_surface, move |state, surface| {
                on_initial_commit(state, window, surface);
            });
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        let wl_surface = surface.wl_surface().clone();

        surface.with_pending_state(|state| {
            // TODO: Proper positioning
            state.geometry = positioner.get_geometry();
        });

        self.popups
            .track_popup(PopupKind::from(surface.clone()))
            .ok();

        self.commit_dispatcher
            .on_next_commit(wl_surface, move |_, _| {
                surface.send_configure().ok();
            });
    }

    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat: Seat<Self> = Seat::from_resource(&seat).unwrap();
        let ret =
            self.popups
                .grab_popup(&self.display, surface.wl_surface().clone(), &seat, serial);

        if let Ok(mut grab) = ret {
            if let Some(keyboard) = seat.get_keyboard() {
                if keyboard.is_grabbed()
                    && !(keyboard.has_grab(serial)
                        || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                keyboard.set_focus(self, grab.current_grab().map(|(_, s)| s), serial);
                keyboard.set_grab(PopupKeyboardGrab::new(&grab), serial);
            }
            if let Some(pointer) = seat.get_pointer() {
                if pointer.is_grabbed()
                    && !(pointer.has_grab(serial)
                        || pointer
                            .has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
            }
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .window_for_surface(wl_surface, WindowSurfaceType::TOPLEVEL)
                .unwrap()
                .clone();
            let initial_window_location = self.space.window_location(&window).unwrap();

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .window_for_surface(wl_surface, WindowSurfaceType::TOPLEVEL)
                .unwrap()
                .clone();
            let initial_window_location = self.space.window_location(&window).unwrap();
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::from_loc_and_size(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }
}

// Xdg Shell
delegate_xdg_shell!(State);

fn check_grab(
    seat: &Seat<State>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<State>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}
