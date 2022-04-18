use smithay::desktop::{self, LayerSurface, PopupKind, PopupManager};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DispatchData, Display};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{self, TraversalAction};
use smithay::wayland::seat::{PointerGrabStartData, Seat};
use smithay::wayland::shell::wlr_layer::{
    wlr_layer_shell_init, Layer, LayerSurfaceAttributes, LayerSurfaceConfigure,
};
use smithay::wayland::shell::xdg::{
    xdg_shell_init, PositionerState, XdgPopupSurfaceRoleAttributes,
};
use smithay::wayland::Serial;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Mutex;

use smithay::desktop::Window;

use super::surface_data::{MoveAfterResizeState, ResizeData, ResizeEdge, ResizeState, SurfaceData};

mod surface_lists;
use surface_lists::{NotMappedList, ShellLayerList, ShellWindowList};

mod wlr_layer;
mod xdg;

mod utils;

#[cfg(feature = "xwayland")]
use {
    smithay::reexports::{calloop::LoopHandle, wayland_server::Client},
    std::os::unix::net::UnixStream,
    std::sync::Arc,
    x11rb::protocol::xproto::{ConfigureRequestEvent, ConfigureWindowAux, ConnectionExt},
};

#[cfg(feature = "xwayland")]
pub struct X11WindowUserData {
    pub window: x11rb::protocol::xproto::Window,
    pub location: Point<i32, Logical>,
}

#[cfg(feature = "xwayland")]
pub mod xwayland;

#[allow(unused)]
pub trait ShellHandler {
    fn window_created(&mut self, window: Window) {}

    fn window_move(
        &mut self,
        window: desktop::Window,
        start_data: PointerGrabStartData,
        seat: Seat,
        serial: Serial,
    ) {
    }

    fn window_resize(
        &mut self,
        window: desktop::Window,
        start_data: PointerGrabStartData,
        seat: Seat,
        edges: ResizeEdge,
        serial: Serial,
    ) {
    }

    fn window_got_resized(
        &mut self,
        window: desktop::Window,
        new_location_x: Option<i32>,
        new_location_y: Option<i32>,
    ) {
    }

    fn window_maximize(&mut self, window: desktop::Window) {}
    fn window_unmaximize(&mut self, window: desktop::Window) {}

    fn window_fullscreen(&mut self, window: desktop::Window, output: Option<WlOutput>) {}
    fn window_unfullscreen(&mut self, window: desktop::Window) {}

    fn window_minimize(&mut self, window: desktop::Window) {}

    //
    // Popup
    //
    fn popup_created(&mut self, popup: PopupKind, positioner: PositionerState) {}

    fn popup_grab(
        &mut self,
        popup: PopupKind,
        start_data: PointerGrabStartData,
        seat: Seat,
        serial: Serial,
    ) {
    }

    //
    // Misc
    //
    fn show_window_menu(
        &mut self,
        window: desktop::Window,
        seat: Seat,
        serial: Serial,
        location: Point<i32, Logical>,
    ) {
    }

    fn surface_commit(&mut self, surface: WlSurface);

    //
    // Wlr Layer Shell
    //
    fn layer_created(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
    }

    fn layer_ack_configure(&mut self, surface: WlSurface, configure: LayerSurfaceConfigure) {}

    #[cfg(feature = "xwayland")]
    fn xwayland_configure_request(
        &mut self,
        conn: Arc<x11rb::rust_connection::RustConnection>,
        event: ConfigureRequestEvent,
    ) {
        // Just grant the wish
        let aux = ConfigureWindowAux::from_configure_request(&event);
        conn.configure_window(event.window, &aux).ok();
    }
}

struct Inner<D> {
    not_mapped_list: NotMappedList,
    windows: ShellWindowList,
    layers: ShellLayerList,

    popup_manager: PopupManager,
    _pd: PhantomData<D>,
}

impl<D> Inner<D>
where
    D: ShellHandler + 'static,
{
    // Try to updated mapped surface
    fn try_update_mapped(&mut self, surface: &WlSurface, handler: &mut D) {
        if let Some(window) = self.windows.find_mut(surface) {
            let geometry = window.geometry();

            let new_location = SurfaceData::with_mut(surface, |data| {
                let mut new_location_x = None;
                let mut new_location_y = None;

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
                            if edges.intersects(ResizeEdge::LEFT) {
                                new_location_x = Some(
                                    initial_window_location.x
                                        + (initial_window_size.w - geometry.size.w),
                                );
                            }
                            if edges.intersects(ResizeEdge::TOP) {
                                new_location_y = Some(
                                    initial_window_location.y
                                        + (initial_window_size.h - geometry.size.h),
                                );
                            }
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
                    new_location_x = Some(mdata.target_window_location.x);
                    new_location_y = Some(mdata.target_window_location.y);

                    data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
                }

                (new_location_x, new_location_y)
            });

            if new_location.0.is_some() | new_location.1.is_some() {
                handler.window_got_resized(window.clone(), new_location.0, new_location.1);
            }
        }
    }

    // Try to map surface
    fn try_map_unmaped(&mut self, surface: &WlSurface, handler: &mut D) {
        if let Some(window) = self.not_mapped_list.try_window_map(surface) {
            self.windows.push(window.clone());
            handler.window_created(window);
        }

        if let Some(popup) = self.popup_manager.find_popup(surface) {
            let PopupKind::Xdg(ref popup) = popup;
            let initial_configure_sent = compositor::with_states(surface, |states| {
                states
                    .data_map
                    .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            })
            .unwrap();
            if !initial_configure_sent {
                popup.send_configure().expect("Initial configure failed");
            }
        }
    }

    fn surface_commit(&mut self, surface: WlSurface, mut ddata: DispatchData) {
        let handler = ddata.get::<D>().unwrap();

        #[cfg(feature = "xwayland")]
        self.xwayland_commit_hook(&surface);

        smithay::backend::renderer::utils::on_commit_buffer_handler(&surface);
        self.popup_manager.commit(&surface);

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
                },
                |_, _, _| true,
            );
        }

        // Map unmaped windows
        self.try_map_unmaped(&surface, handler);

        // Update mapped windows
        self.try_update_mapped(&surface, handler);

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
                layer.layer_surface().send_configure();
            }
        }

        let handler = ddata.get::<D>().unwrap();
        handler.surface_commit(surface);
    }
}

pub struct ShellManager<D> {
    inner: Rc<RefCell<Inner<D>>>,
}

impl<D> ShellManager<D>
where
    D: ShellHandler + 'static,
{
    pub fn init_shell(display: &mut Display) -> Self
    where
        D: ShellHandler + 'static,
    {
        let inner = Rc::new(RefCell::new(Inner {
            not_mapped_list: Default::default(),
            windows: Default::default(),
            layers: Default::default(),

            popup_manager: PopupManager::new(None),
            _pd: PhantomData::<D>,
        }));

        // Create the compositor
        compositor::compositor_init(
            display,
            {
                let inner = inner.clone();
                move |surface, ddata| inner.borrow_mut().surface_commit(surface, ddata)
            },
            None,
        );

        // init the xdg_shell
        xdg_shell_init(
            display,
            {
                let inner = inner.clone();
                move |request, mut ddata| {
                    inner
                        .borrow_mut()
                        .xdg_shell_request(request, ddata.get().unwrap());
                }
            },
            None,
        );

        // init the wlr_layer_shell
        wlr_layer_shell_init(
            display,
            {
                let inner = inner.clone();
                move |request, mut ddata| {
                    inner
                        .borrow_mut()
                        .wlr_layer_shell_request(request, ddata.get().unwrap())
                }
            },
            None,
        );

        Self { inner }
    }

    #[cfg(feature = "xwayland")]
    pub fn xwayland_ready(
        &mut self,
        handle: &LoopHandle<D>,
        connection: UnixStream,
        client: Client,
    ) {
        xwayland::xwayland_shell_init(handle, connection, client, {
            let inner = self.inner.clone();

            move |event, x11, client, mut ddata| {
                let handler = ddata.get::<D>().unwrap();

                inner
                    .borrow_mut()
                    .xwayland_shell_event(event, x11, client, handler)
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
