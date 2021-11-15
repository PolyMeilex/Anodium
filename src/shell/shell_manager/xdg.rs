use std::sync::Mutex;

use smithay::{
    reexports::{wayland_protocols::xdg_shell::server::xdg_toplevel, wayland_server::DispatchData},
    wayland::{
        compositor,
        seat::{GrabStartData, Seat},
        shell::xdg::{Configure, ToplevelSurface, XdgRequest, XdgToplevelSurfaceRoleAttributes},
        Serial,
    },
};

use crate::{
    desktop_layout::WindowSurface,
    shell::{surface_data::ResizeState, MoveAfterResizeState, SurfaceData},
};

use super::{Inner, ShellEvent};

impl Inner {
    pub(super) fn xdg_shell_request(&mut self, request: XdgRequest, ddata: DispatchData) {
        match request {
            //
            // Toplevel
            //
            XdgRequest::NewToplevel { surface } => {
                self.not_mapped_list
                    .insert(WindowSurface::Xdg(surface), Default::default());
            }

            XdgRequest::Move {
                seat,
                serial,
                surface,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    (self.cb)(
                        ShellEvent::ViewMove {
                            toplevel: WindowSurface::Xdg(surface),
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
                        ShellEvent::ViewResize {
                            toplevel: WindowSurface::Xdg(surface),
                            start_data,
                            seat,
                            edges,
                            serial,
                        },
                        ddata,
                    );
                }
            }

            XdgRequest::Maximize { surface } => {
                (self.cb)(
                    ShellEvent::ViewMaximize {
                        toplevel: WindowSurface::Xdg(surface),
                    },
                    ddata,
                );
            }
            XdgRequest::UnMaximize { surface } => {
                (self.cb)(
                    ShellEvent::ViewUnMaximize {
                        toplevel: WindowSurface::Xdg(surface),
                    },
                    ddata,
                );
            }

            XdgRequest::Fullscreen { surface, output } => {
                (self.cb)(
                    ShellEvent::ViewFullscreen {
                        toplevel: WindowSurface::Xdg(surface),
                        output,
                    },
                    ddata,
                );
            }
            XdgRequest::UnFullscreen { surface } => {
                (self.cb)(
                    ShellEvent::ViewUnFullscreen {
                        toplevel: WindowSurface::Xdg(surface),
                    },
                    ddata,
                );
            }

            XdgRequest::Minimize { surface } => {
                (self.cb)(
                    ShellEvent::ViewMinimize {
                        toplevel: WindowSurface::Xdg(surface),
                    },
                    ddata,
                );
            }

            //
            // Popup
            //
            XdgRequest::NewPopup { .. } => todo!(),
            XdgRequest::Grab { .. } => todo!(),
            XdgRequest::RePosition { .. } => todo!(),

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
                        toplevel: WindowSurface::Xdg(surface),
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
                })
                .unwrap_or_default();

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

fn check_grab(seat: &Seat, serial: Serial, surface: &ToplevelSurface) -> Option<GrabStartData> {
    let surface = surface.get_surface()?;
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
