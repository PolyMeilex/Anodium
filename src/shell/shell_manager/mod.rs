use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DispatchData, Display};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{self, SurfaceAttributes, TraversalAction};
use smithay::wayland::seat::{GrabStartData, Seat};
use smithay::wayland::shell::wlr_layer::{
    wlr_layer_shell_init, Layer, LayerSurfaceAttributes, LayerSurfaceConfigure,
};
use smithay::wayland::shell::xdg::xdg_shell_init;
use smithay::wayland::Serial;
use std::cell::RefCell;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::Mutex;

use crate::desktop_layout::{LayerSurface, Window, WindowSurface};
use crate::state::BackendState;
use crate::utils::LogResult;

use super::surface_data::{ResizeData, ResizeEdge, ResizeState};
use super::{MoveAfterResizeState, SurfaceData};

mod surface_lists;
use surface_lists::{NotMappedList, ShellLayerList, ShellWindowList};

mod wlr_layer;
mod xdg;

#[cfg(feature = "xwayland")]
pub mod xwayland;
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

    SurfaceCommit {
        surface: WlSurface,
    },

    //
    // Wlr Layer Shell
    //
    LayerCreated {
        surface: LayerSurface,
        output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    },
    LayerAckConfigure {
        surface: WlSurface,
        configure: LayerSurfaceConfigure,
    },
}

struct Inner {
    cb: Box<dyn FnMut(ShellEvent, DispatchData)>,
    not_mapped_list: NotMappedList,
    windows: ShellWindowList,
    layers: ShellLayerList,
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
            .expect("Commited surface without SurfaceData");

            if let Some(location) = new_location {
                window.set_location(location);
            }
        }
    }

    // Try to map surface
    fn try_map_unmaped(&mut self, surface: &WlSurface, ddata: DispatchData) {
        if let Some(window) = self.not_mapped_list.try_map(&surface) {
            self.windows.push(window.clone());
            (self.cb)(ShellEvent::WindowCreated { window }, ddata);
        }
    }

    fn surface_commit(&mut self, surface: WlSurface, mut ddata: DispatchData) {
        #[cfg(feature = "xwayland")]
        self.xwayland_commit_hook(&surface);

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
        self.try_map_unmaped(&surface, ddata.reborrow());

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

        if let Some(layer) = self.layers.find(&surface) {
            let initial_configure_sent = compositor::with_states(&surface, |states| {
                states
                    .data_map
                    .get::<Mutex<LayerSurfaceAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            })
            .unwrap();
            if !initial_configure_sent {
                layer.surface().send_configure();
            }
        }

        (self.cb)(ShellEvent::SurfaceCommit { surface }, ddata);
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
            layers: Default::default(),
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

        // init the wlr_layer_shell
        wlr_layer_shell_init(
            display,
            {
                let inner = inner.clone();
                move |request, ddata| inner.borrow_mut().wlr_layer_shell_request(request, ddata)
            },
            slog_scope::logger(),
        );

        Self { inner }
    }

    #[cfg(feature = "xwayland")]
    pub fn xwayland_ready(
        &mut self,
        handle: &LoopHandle<BackendState>,
        connection: UnixStream,
        client: Client,
    ) {
        xwayland::xwayland_shell_init(handle, connection, client, {
            let inner = self.inner.clone();

            move |event, x11, client, ddata| {
                inner
                    .borrow_mut()
                    .xwayland_shell_event(event, x11, client, ddata)
                    .log_err("Error while handling X11 event:")
                    .ok();
            }
        });
    }

    pub fn refresh(&mut self) {
        self.inner.borrow_mut().windows.refresh();
        self.inner.borrow_mut().layers.refresh();
        self.inner.borrow_mut().not_mapped_list.refresh();
    }
}
