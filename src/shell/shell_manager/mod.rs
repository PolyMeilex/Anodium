use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DispatchData, Display};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{self, SurfaceAttributes, TraversalAction};
use smithay::wayland::seat::{GrabStartData, Seat};
use smithay::wayland::shell::xdg::xdg_shell_init;
use smithay::wayland::Serial;
use std::cell::RefCell;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use crate::desktop_layout::{Window, WindowSurface};
use crate::state::BackendState;

use super::not_mapped_list::NotMappedList;
use super::surface_data::{ResizeData, ResizeEdge, ResizeState};
use super::{MoveAfterResizeState, SurfaceData};

mod window_list;
use window_list::ShellWindowList;

mod xdg;

#[cfg(feature = "xwayland")]
pub mod xwayland;
#[cfg(feature = "xwayland")]
use xwayland::X11State;
#[cfg(feature = "xwayland")]
pub use xwayland::X11Surface;

pub enum ShellEvent {
    WindowCreated {
        window: Window,
    },

    WindowMove {
        toplevel: WindowSurface,
        start_data: GrabStartData,
        seat: Seat,
        serial: Serial,
    },

    WindowResize {
        toplevel: WindowSurface,
        start_data: GrabStartData,
        seat: Seat,
        edges: ResizeEdge,
        serial: Serial,
    },

    WindowMaximize {
        toplevel: WindowSurface,
    },
    WindowUnMaximize {
        toplevel: WindowSurface,
    },

    WindowFullscreen {
        toplevel: WindowSurface,
        output: Option<WlOutput>,
    },
    WindowUnFullscreen {
        toplevel: WindowSurface,
    },

    WindowMinimize {
        toplevel: WindowSurface,
    },

    //
    // Misc
    //
    ShowWindowMenu {
        toplevel: WindowSurface,
        seat: Seat,
        serial: Serial,
        location: Point<i32, Logical>,
    },
}

struct Inner {
    cb: Box<dyn FnMut(ShellEvent, DispatchData)>,
    not_mapped_list: NotMappedList,
    windows: ShellWindowList,
}

impl Inner {
    // Try to updated mapped surface
    fn try_update_mapped(&mut self, surface: &WlSurface) {
        if let Some(window) = self.windows.find_mut(surface) {
            window.self_update();

            let geometry = window.geometry();
            let new_location = SurfaceData::with_mut(&surface, |data| {
                let mut new_location = None;

                // If the window is being resized by top or left, its location must be adjusted
                // accordingly.
                match data.resize_state {
                    ResizeState::Resizing(resize_data)
                    | ResizeState::WaitingForFinalAck(resize_data, _)
                    | ResizeState::WaitingForCommit(resize_data) => {
                        let ResizeData {
                            edges,
                            initial_window_location,
                            initial_window_size,
                        } = resize_data;

                        if edges.intersects(ResizeEdge::TOP_LEFT) {
                            let mut location = window.location();

                            if edges.intersects(ResizeEdge::LEFT) {
                                location.x = initial_window_location.x
                                    + (initial_window_size.w - geometry.size.w);
                            }
                            if edges.intersects(ResizeEdge::TOP) {
                                location.y = initial_window_location.y
                                    + (initial_window_size.h - geometry.size.h);
                            }

                            new_location = Some(location);
                        }
                    }
                    ResizeState::NotResizing => (),
                }

                // Finish resizing.
                if let ResizeState::WaitingForCommit(_) = data.resize_state {
                    data.resize_state = ResizeState::NotResizing;
                }

                // If the compositor requested MoveAfterReszie
                if let MoveAfterResizeState::WaitingForCommit(mdata) = data.move_after_resize_state
                {
                    new_location = Some(mdata.target_window_location);
                    data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
                }

                new_location
            })
            .unwrap();

            if let Some(location) = new_location {
                window.set_location(location);
            }
        }
    }

    fn surface_commit(&mut self, surface: WlSurface, ddata: DispatchData) {
        #[cfg(feature = "xwayland")]
        xwayland::commit_hook(&surface);

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
        if let Some(window) = self.not_mapped_list.try_map(&surface) {
            self.windows.push(window.clone());
            (self.cb)(ShellEvent::WindowCreated { window }, ddata);
        }

        // Update mapped windows
        self.try_update_mapped(&surface);

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
    inner: Rc<RefCell<Inner>>,
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
            windows: Default::default(),
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

        Self { inner }
    }

    pub fn refresh(&mut self) {
        self.inner.borrow_mut().windows.refresh();
    }

    #[cfg(feature = "xwayland")]
    pub fn xwayland_ready(
        &mut self,
        handle: &LoopHandle<BackendState>,
        connection: UnixStream,
        client: Client,
    ) {
        let (x11_state, source) = X11State::start_wm(connection, {
            let inner = self.inner.clone();
            // Listen for new windows
            move |window_surface, location| {
                inner
                    .borrow_mut()
                    .not_mapped_list
                    .insert(window_surface, location);
            }
        })
        .unwrap();

        let x11_state = Rc::new(RefCell::new(x11_state));
        client
            .data_map()
            .insert_if_missing(|| Rc::clone(&x11_state));

        handle
            .insert_source(source, move |event, _, _| {
                match x11_state.borrow_mut().handle_event(event, &client) {
                    Ok(()) => {}
                    Err(err) => error!("Error while handling X11 event: {}", err),
                }
            })
            .unwrap();
    }
}
