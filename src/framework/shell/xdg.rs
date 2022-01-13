use std::sync::Mutex;

use smithay::{
    desktop::Kind,
    reexports::{wayland_protocols::xdg_shell::server::xdg_toplevel, wayland_server::DispatchData},
    wayland::{
        compositor,
        seat::{GrabStartData, Seat},
        shell::xdg::{Configure, XdgRequest, XdgToplevelSurfaceRoleAttributes},
        Serial,
    },
};

use super::super::surface_data::{MoveAfterResizeState, ResizeState, SurfaceData};

use crate::{popup::PopupSurface, utils::AsWlSurface, window::WindowSurface};

use super::ShellEvent;

impl super::Inner {
    pub fn xdg_shell_request(&mut self, request: XdgRequest, ddata: DispatchData) {
        match request {
            //
            // Toplevel
            //
            XdgRequest::NewToplevel { surface } => {
                self.not_mapped_list
                    .insert_window(WindowSurface::Xdg(surface), Default::default());
            }

            XdgRequest::Move {
                seat,
                serial,
                surface,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    (self.cb)(
                        ShellEvent::WindowMove {
                            toplevel: Kind::Xdg(surface),
                            start_data,
                            seat,
                            serial,
                        },
                        ddata,
                    );
                }
            }
            XdgRequest::Resize {
                surface,
                seat,
                serial,
                edges,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    (self.cb)(
                        ShellEvent::WindowResize {
                            toplevel: Kind::Xdg(surface),
                            start_data,
                            seat,
                            edges: edges.into(),
                            serial,
                        },
                        ddata,
                    );
                }
            }

            XdgRequest::Maximize { surface } => {
                (self.cb)(
                    ShellEvent::WindowMaximize {
                        toplevel: Kind::Xdg(surface),
                    },
                    ddata,
                );
            }
            XdgRequest::UnMaximize { surface } => {
                (self.cb)(
                    ShellEvent::WindowUnMaximize {
                        toplevel: Kind::Xdg(surface),
                    },
                    ddata,
                );
            }

            XdgRequest::Fullscreen { surface, output } => {
                (self.cb)(
                    ShellEvent::WindowFullscreen {
                        toplevel: Kind::Xdg(surface),
                        output,
                    },
                    ddata,
                );
            }
            XdgRequest::UnFullscreen { surface } => {
                (self.cb)(
                    ShellEvent::WindowUnFullscreen {
                        toplevel: Kind::Xdg(surface),
                    },
                    ddata,
                );
            }

            XdgRequest::Minimize { surface } => {
                (self.cb)(
                    ShellEvent::WindowMinimize {
                        toplevel: Kind::Xdg(surface),
                    },
                    ddata,
                );
            }

            //
            // Popup
            //
            XdgRequest::NewPopup { surface, .. } => {
                self.not_mapped_list
                    .insert_popup(PopupSurface::Xdg(surface));
            }
            XdgRequest::Grab {
                seat,
                serial,
                surface,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    (self.cb)(
                        ShellEvent::PopupGrab {
                            popup: PopupSurface::Xdg(surface),
                            start_data,
                            seat,
                            serial,
                        },
                        ddata,
                    );
                }
            }
            XdgRequest::RePosition { .. } => {
                error!("TODO: Popup RePosition");
            }

            //
            // Misc
            //
            XdgRequest::ShowWindowMenu {
                surface,
                seat,
                serial,
                location,
            } => {
                (self.cb)(
                    ShellEvent::ShowWindowMenu {
                        toplevel: Kind::Xdg(surface),
                        seat: Seat::from_resource(&seat).unwrap(),
                        serial,
                        location,
                    },
                    ddata,
                );
            }

            XdgRequest::AckConfigure {
                surface,
                configure: Configure::Toplevel(configure),
            } => {
                let waiting_for_serial = SurfaceData::with(&surface, |data| {
                    if let ResizeState::WaitingForFinalAck(_, serial) = data.resize_state {
                        Some(serial)
                    } else {
                        None
                    }
                });

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
                    let is_resizing = compositor::with_states(&surface, |states| {
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
                        SurfaceData::with_mut(&surface, |data| {
                            if let ResizeState::WaitingForFinalAck(resize_data, _) =
                                data.resize_state
                            {
                                data.resize_state = ResizeState::WaitingForCommit(resize_data);
                            } else {
                                unreachable!()
                            }
                        });
                    }
                }

                // Maximize / Fullscreen
                SurfaceData::with_mut(&surface, |data| {
                    if let MoveAfterResizeState::WaitingForAck(mdata) = data.move_after_resize_state
                    {
                        data.move_after_resize_state =
                            MoveAfterResizeState::WaitingForCommit(mdata);
                    }
                });
            }
            _ => {}
        }
    }
}

fn check_grab<S: AsWlSurface>(seat: &Seat, serial: Serial, surface: &S) -> Option<GrabStartData> {
    let surface = surface.as_surface()?;
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if pointer.has_grab(serial) {
        let start_data = pointer.grab_start_data()?;
        let focus = start_data.focus.as_ref()?;

        if focus.0.as_ref().same_client_as(surface.as_ref()) {
            Some(start_data)
        } else {
            // If the focus was for a different surface, ignore the request.
            None
        }
    } else {
        None
    }
}
