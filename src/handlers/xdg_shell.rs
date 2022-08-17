use smithay::{
    delegate_xdg_shell,
    desktop::{
        Kind, PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window,
        WindowSurfaceType,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            DisplayHandle, Resource,
        },
    },
    utils::Rectangle,
    wayland::{
        seat::{Focus, PointerGrabStartData, Seat},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        Serial,
    },
};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    State,
};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, _dh: &DisplayHandle, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface().clone();

        let window = Window::new(Kind::Xdg(surface));
        self.space.map_window(&window, (0, 0), None, false);

        self.on_commit_dispatcher
            .on_next_commit(wl_surface, move |_, _| {
                window.configure();
            });
    }

    fn new_popup(
        &mut self,
        _dh: &DisplayHandle,
        surface: PopupSurface,
        positioner: PositionerState,
    ) {
        let wl_surface = surface.wl_surface().clone();

        surface.with_pending_state(|state| {
            // TODO: Proper positioning
            state.geometry = positioner.get_geometry();
        });

        self.popups
            .track_popup(PopupKind::from(surface.clone()))
            .ok();

        self.on_commit_dispatcher
            .on_next_commit(wl_surface, move |_, _| {
                surface.send_configure().ok();
            });
    }

    fn grab(
        &mut self,
        dh: &DisplayHandle,
        surface: PopupSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
    ) {
        let seat: Seat<Self> = Seat::from_resource(&seat).unwrap();
        let ret = self.popups.grab_popup(dh, surface.into(), &seat, serial);

        if let Ok(mut grab) = ret {
            if let Some(keyboard) = seat.get_keyboard() {
                if keyboard.is_grabbed()
                    && !(keyboard.has_grab(serial)
                        || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
                {
                    grab.ungrab(dh, PopupUngrabStrategy::All);
                    return;
                }
                keyboard.set_focus(dh, grab.current_grab().as_ref(), serial);
                keyboard.set_grab(PopupKeyboardGrab::new(&grab), serial);
            }
            if let Some(pointer) = seat.get_pointer() {
                if pointer.is_grabbed()
                    && !(pointer.has_grab(serial)
                        || pointer
                            .has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
                {
                    grab.ungrab(dh, PopupUngrabStrategy::All);
                    return;
                }
                pointer.set_grab(PopupPointerGrab::new(&grab), serial, Focus::Keep);
            }
        }
    }

    fn move_request(
        &mut self,
        _dh: &DisplayHandle,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
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

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        _dh: &DisplayHandle,
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

            pointer.set_grab(grab, serial, Focus::Clear);
        }
    }
}

// Xdg Shell
delegate_xdg_shell!(State);

fn check_grab(
    seat: &Seat<State>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData> {
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
