use std::{cell::RefCell, sync::Mutex};

use smithay::{
    reexports::wayland_protocols::xdg_shell::server::xdg_toplevel,
    wayland::{
        compositor::with_states,
        seat::Seat,
        shell::xdg::{Configure, XdgRequest, XdgToplevelSurfaceRoleAttributes},
    },
};

use crate::{
    desktop_layout::{GrabState, Toplevel},
    state::MainState,
};

use super::{
    move_surface_grab::MoveSurfaceGrab,
    surface_data::{ResizeState, SurfaceData},
    MaximizeState,
};

impl MainState {
    pub fn xdg_shell_request(&mut self, request: XdgRequest) {
        match request {
            XdgRequest::NewToplevel { surface } => {
                self.not_mapped_list
                    .borrow_mut()
                    .insert(Toplevel::Xdg(surface), Default::default());
            }
            // TODO:
            // XdgRequest::NewPopup { surface } => {
            // self.window_map
            //     .borrow_mut()
            //     .popups_mut()
            //     .insert(PopupKind::Xdg(surface));
            // }
            XdgRequest::Move {
                surface,
                seat,
                serial,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();
                let pointer = seat.get_pointer().unwrap();

                // Check that this surface has a click grab.
                if !pointer.has_grab(serial) {
                    return;
                }

                let start_data = pointer.grab_start_data().unwrap();

                // If the focus was for a different surface, ignore the request.
                if start_data.focus.is_none()
                    || !start_data
                        .focus
                        .as_ref()
                        .unwrap()
                        .0
                        .as_ref()
                        .same_client_as(surface.get_surface().unwrap().as_ref())
                {
                    return;
                }

                let toplevel = Toplevel::Xdg(surface);

                let mut desktop_layout = self.desktop_layout.borrow_mut();

                if let Some(space) = desktop_layout.find_workspace_by_surface_mut(&toplevel) {
                    if let Some(res) = space.move_request(&toplevel, &seat, serial, &start_data) {
                        if let Some(window) = space.unmap_toplevel(&toplevel) {
                            desktop_layout.grabed_window = Some(GrabState { window, done: false });

                            let grab = MoveSurfaceGrab {
                                start_data,
                                toplevel,
                                initial_window_location: res.initial_window_location,
                                desktop_layout: self.desktop_layout.clone(),
                            };
                            pointer.set_grab(grab, serial);
                        }
                    }
                }
            }
            XdgRequest::Resize {
                surface,
                seat,
                serial,
                edges,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();
                let pointer = seat.get_pointer().unwrap();

                // Check that this surface has a click grab.
                if !pointer.has_grab(serial) {
                    return;
                }

                let start_data = pointer.grab_start_data().unwrap();

                // If the focus was for a different surface, ignore the request.
                if start_data.focus.is_none()
                    || !start_data
                        .focus
                        .as_ref()
                        .unwrap()
                        .0
                        .as_ref()
                        .same_client_as(surface.get_surface().unwrap().as_ref())
                {
                    return;
                }

                let toplevel = Toplevel::Xdg(surface.clone());
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.resize_request(&toplevel, &seat, serial, start_data, edges);
                }
            }
            XdgRequest::Maximize { surface } => {
                let toplevel = Toplevel::Xdg(surface.clone());
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.maximize_request(&toplevel);
                }
            }
            XdgRequest::UnMaximize { surface } => {
                let toplevel = Toplevel::Xdg(surface.clone());
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.unmaximize_request(&toplevel);
                }
            }
            XdgRequest::AckConfigure {
                surface,
                configure: Configure::Toplevel(configure),
                ..
            } => {
                let waiting_for_serial = with_states(&surface, |states| {
                    if let Some(data) = states.data_map.get::<RefCell<SurfaceData>>() {
                        if let ResizeState::WaitingForFinalAck(_, serial) = data.borrow().resize_state {
                            return Some(serial);
                        }
                    }

                    None
                })
                .unwrap();

                if let Some(serial) = waiting_for_serial {
                    // When the resize grab is released the surface
                    // resize state will be set to WaitingForFinalAck
                    // and the client will receive a configure request
                    // without the resize state to inform the client
                    // resizing has finished. Here we will wait for
                    // the client to acknowledge the end of the
                    // resizing. To check if the surface was resizing
                    // before sending the configure we need to use
                    // the current state as the received acknowledge
                    // will no longer have the resize state set
                    let is_resizing = with_states(&surface, |states| {
                        states
                            .data_map
                            .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                            .unwrap()
                            .lock()
                            .unwrap()
                            .current
                            .states
                            .contains(xdg_toplevel::State::Resizing)
                    })
                    .unwrap();

                    if configure.serial >= serial && is_resizing {
                        with_states(&surface, |states| {
                            let mut data = states
                                .data_map
                                .get::<RefCell<SurfaceData>>()
                                .unwrap()
                                .borrow_mut();
                            if let ResizeState::WaitingForFinalAck(resize_data, _) = data.resize_state {
                                data.resize_state = ResizeState::WaitingForCommit(resize_data);
                            } else {
                                unreachable!()
                            }
                        })
                        .unwrap();
                    }
                }

                // Maximize / Fullscreen
                with_states(&surface, |states| {
                    if let Some(data) = states.data_map.get::<RefCell<SurfaceData>>() {
                        let mut data = data.borrow_mut();
                        if let MaximizeState::WaitingForFinalAck(mdata) = data.maximize_state {
                            data.maximize_state = MaximizeState::WaitingForCommit(mdata);
                        }
                    }
                })
                .unwrap();
            }
            _ => {}
        }
    }
}
