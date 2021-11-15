use smithay::reexports::wayland_protocols::xdg_shell::server::xdg_toplevel::ResizeEdge;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::{self, WlSurface};
use smithay::reexports::wayland_server::{DispatchData, Display};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{self, SurfaceAttributes, TraversalAction};
use smithay::wayland::seat::{GrabStartData, Seat};
use smithay::wayland::shell::xdg::{xdg_shell_init, XdgRequest, XdgToplevelSurfaceRoleAttributes};
use smithay::wayland::Serial;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use crate::desktop_layout::{Toplevel, Window};

use super::not_mapped_list::NotMappedList;
use super::SurfaceData;

mod xdg;

pub enum ShellEvent {
    ViewCreated {
        window: Window,
    },

    ViewMove {
        toplevel: Toplevel,
        start_data: GrabStartData,
        seat: Seat,
        serial: Serial,
    },

    ViewResize {
        toplevel: Toplevel,
        start_data: GrabStartData,
        seat: Seat,
        edges: ResizeEdge,
        serial: Serial,
    },

    ViewMaximize {
        toplevel: Toplevel,
    },
    ViewUnMaximize {
        toplevel: Toplevel,
    },

    ViewFullscreen {
        toplevel: Toplevel,
        output: Option<WlOutput>,
    },
    ViewUnFullscreen {
        toplevel: Toplevel,
    },

    ViewMinimize {
        toplevel: Toplevel,
    },

    //
    // Misc
    //
    ShowWindowMenu {
        toplevel: Toplevel,
        seat: Seat,
        serial: Serial,
        location: Point<i32, Logical>,
    },
}

struct Inner {
    cb: Box<dyn FnMut(ShellEvent, DispatchData)>,
    not_mapped_list: NotMappedList,
}

impl Inner {
    fn try_map_unmaped(&mut self, surface: &WlSurface, ddata: DispatchData) {
        if let Some(win) = self.not_mapped_list.find_mut(surface) {
            win.self_update();

            let toplevel = win.toplevel().clone();
            // send the initial configure if relevant
            if let Toplevel::Xdg(ref toplevel) = toplevel {
                let initial_configure_sent = compositor::with_states(surface, |states| {
                    states
                        .data_map
                        .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                })
                .unwrap();
                if !initial_configure_sent {
                    toplevel.send_configure();
                }
            }

            let size = win.geometry().size;
            if size.w != 0 && size.h != 0 {
                match toplevel {
                    Toplevel::Xdg(_) => {
                        let configured = compositor::with_states(surface, |states| {
                            states
                                .data_map
                                .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                                .unwrap()
                                .lock()
                                .unwrap()
                                .configured
                        })
                        .unwrap();

                        if configured {
                            let pending = self.not_mapped_list.remove(&toplevel);

                            if let Some(window) = pending {
                                (self.cb)(ShellEvent::ViewCreated { window }, ddata);
                            }
                        }
                    }
                    #[cfg(feature = "xwayland")]
                    Toplevel::X11(_) => {
                        let pending = self.not_mapped_list.remove(&toplevel);

                        if let Some(window) = pending {
                            (self.cb)(ShellEvent::ViewCreated { window }, ddata);
                        }
                    }
                }
            }
        }
    }

    fn surface_commit(&mut self, surface: WlSurface, ddata: DispatchData) {
        #[cfg(feature = "xwayland")]
        crate::xwayland::commit_hook(&surface);

        if !compositor::is_sync_subsurface(&surface) {
            // Update the buffer of all child surfaces
            compositor::with_surface_tree_upward(
                &surface,
                (),
                |_, _, _| TraversalAction::DoChildren(()),
                |_, states, _| {
                    states
                        .data_map
                        .insert_if_missing(|| RefCell::new(SurfaceData::default()));
                    let mut data = states
                        .data_map
                        .get::<RefCell<SurfaceData>>()
                        .unwrap()
                        .borrow_mut();
                    data.update_buffer(&mut *states.cached_state.current::<SurfaceAttributes>());
                },
                |_, _, _| true,
            );
        }

        // Map unmaped windows
        self.try_map_unmaped(&surface, ddata);

        // Update maped windows
        // {
        // In visible workspaces
        // for workspace in self.desktop_layout.borrow_mut().visible_workspaces_mut() {
        //     if let Some(window) = workspace.find_window_mut(surface) {
        //         window.self_update();

        //         let geometry = window.geometry();
        //         let new_location = with_states(surface, |states| {
        //             let mut data = states
        //                 .data_map
        //                 .get::<RefCell<SurfaceData>>()
        //                 .unwrap()
        //                 .borrow_mut();

        //             let mut new_location = None;

        //             // If the window is being resized by top or left, its location must be adjusted
        //             // accordingly.
        //             match data.resize_state {
        //                 ResizeState::Resizing(resize_data)
        //                 | ResizeState::WaitingForFinalAck(resize_data, _)
        //                 | ResizeState::WaitingForCommit(resize_data) => {
        //                     let ResizeData {
        //                         edges,
        //                         initial_window_location,
        //                         initial_window_size,
        //                     } = resize_data;

        //                     if edges.intersects(ResizeEdge::TOP_LEFT) {
        //                         let mut location = window.location();

        //                         if edges.intersects(ResizeEdge::LEFT) {
        //                             location.x = initial_window_location.x
        //                                 + (initial_window_size.w - geometry.size.w);
        //                         }
        //                         if edges.intersects(ResizeEdge::TOP) {
        //                             location.y = initial_window_location.y
        //                                 + (initial_window_size.h - geometry.size.h);
        //                         }

        //                         new_location = Some(location);
        //                     }
        //                 }
        //                 ResizeState::NotResizing => (),
        //             }

        //             // Finish resizing.
        //             if let ResizeState::WaitingForCommit(_) = data.resize_state {
        //                 data.resize_state = ResizeState::NotResizing;
        //             }

        //             // If the compositor requested MoveAfterReszie
        //             if let MoveAfterResizeState::WaitingForCommit(mdata) =
        //                 data.move_after_resize_state
        //             {
        //                 new_location = Some(mdata.target_window_location);
        //                 data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
        //             }

        //             new_location
        //         })
        //         .unwrap();

        //         if let Some(location) = new_location {
        //             window.set_location(location);
        //         }
        //     }
        // }

        // Update currently grabed window
        // if let Some(grab) = self.desktop_layout.borrow().grabed_window.as_ref() {
        //     if let Some(s) = grab.toplevel().get_surface() {
        //         if s == surface {
        //             with_states(surface, |states| {
        //                 let mut data = states
        //                     .data_map
        //                     .get::<RefCell<SurfaceData>>()
        //                     .unwrap()
        //                     .borrow_mut();

        //                 // If the compositor requested MoveAfterReszie
        //                 if let MoveAfterResizeState::WaitingForCommit(mdata) =
        //                     data.move_after_resize_state
        //                 {
        //                     data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
        //                 }
        //             })
        //             .unwrap();
        //         }
        //     }
        // }
        // }

        // TODO:
        // if let Some(popup) = self.window_map.borrow().popups().find(surface) {
        //     let PopupKind::Xdg(ref popup) = popup.popup;
        //     let initial_configure_sent = with_states(surface, |states| {
        //         states
        //             .data_map
        //             .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
        //             .unwrap()
        //             .lock()
        //             .unwrap()
        //             .initial_configure_sent
        //     })
        //     .unwrap();
        //     if !initial_configure_sent {
        //         // TODO: properly recompute the geometry with the whole of positioner state
        //         popup.send_configure();
        //     }
        // }

        // let found = self.desktop_layout.borrow().output_map.iter().any(|o| {
        //     let layer = o.layer_map().find(surface);

        //     if let Some(layer) = layer.as_ref() {
        //         let initial_configure_sent = with_states(surface, |states| {
        //             states
        //                 .data_map
        //                 .get::<Mutex<LayerSurfaceAttributes>>()
        //                 .unwrap()
        //                 .lock()
        //                 .unwrap()
        //                 .initial_configure_sent
        //         })
        //         .unwrap();
        //         if !initial_configure_sent {
        //             layer.surface.send_configure();
        //         }
        //     }

        //     layer.is_some()
        // });

        // if found {
        //     self.desktop_layout.borrow_mut().arrange_layers();
        // }
    }
}

pub struct ShellManager {
    _inner: Rc<RefCell<Inner>>,
}

impl ShellManager {
    pub fn init_shell<F>(display: &mut Display, cb: F) -> Self
    where
        F: FnMut(ShellEvent, DispatchData) + 'static,
    {
        let cb = Box::new(cb);
        let inner = Rc::new(RefCell::new(Inner {
            cb,
            not_mapped_list: Default::default(),
        }));

        // Create the compositor
        compositor::compositor_init(
            display,
            {
                let inner = inner.clone();
                move |surface, ddata| inner.borrow_mut().surface_commit(surface, ddata)
            },
            slog_scope::logger(),
        );

        // init the xdg_shell
        xdg_shell_init(
            display,
            {
                let inner = inner.clone();
                move |request, ddata| inner.borrow_mut().xdg_shell_request(request, ddata)
            },
            slog_scope::logger(),
        );

        // wlr_layer_shell_init(display, move |request, mut ddata| {}, slog_scope::logger());

        Self { _inner: inner }
    }
}
