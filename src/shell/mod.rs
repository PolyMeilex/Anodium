use std::{cell::RefCell, rc::Rc, sync::Mutex};

use smithay::{
    reexports::wayland_server::{protocol::wl_surface, Display},
    wayland::{
        compositor::{
            compositor_init, is_sync_subsurface, with_states, with_surface_tree_upward, SurfaceAttributes,
            TraversalAction,
        },
        shell::{
            wlr_layer::{wlr_layer_shell_init, LayerShellRequest},
            xdg::{xdg_shell_init, XdgToplevelSurfaceRoleAttributes},
        },
    },
};

use crate::{
    desktop_layout::Toplevel,
    state::{Anodium, BackendState},
};

pub mod move_surface_grab;
pub mod not_mapped_list;
pub mod resize_surface_grab;

pub mod surface_data;
pub use surface_data::SurfaceData;
pub use surface_data::{MoveAfterResizeData, MoveAfterResizeState};
use surface_data::{ResizeData, ResizeEdge, ResizeState};

mod xdg_shell;

impl Anodium {
    fn wlr_layer_shell_request(&mut self, request: LayerShellRequest) {
        match request {
            LayerShellRequest::NewLayerSurface {
                // surface,
                // output,
                // layer,
                ..
            } => {
                // TODO:
                // let output_map = self.output_map.borrow();

                // let output = output.and_then(|output| output_map.find_by_output(&output));
                // let output = output.unwrap_or_else(|| {
                //     output_map
                //         .find_by_position(self.pointer_location().to_i32_round())
                //         .unwrap_or_else(|| output_map.with_primary().unwrap())
                // });

                // if let Some(wl_surface) = surface.get_surface() {
                // output.add_layer_surface(wl_surface.clone());
                // self.window_map.borrow_mut().layers.insert(surface, layer);
                // }
            }
            LayerShellRequest::AckConfigure { .. } => {}
        }
    }

    fn surface_commit(&mut self, surface: &wl_surface::WlSurface) {
        #[cfg(feature = "xwayland")]
        super::xwayland::commit_hook(surface);

        if !is_sync_subsurface(surface) {
            // Update the buffer of all child surfaces
            with_surface_tree_upward(
                surface,
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
        {
            let mut not_mapped_list = self.not_mapped_list.borrow_mut();
            if let Some(win) = not_mapped_list.find_mut(surface) {
                win.self_update();

                let toplevel = win.toplevel().clone();
                // send the initial configure if relevant
                if let Toplevel::Xdg(ref toplevel) = toplevel {
                    let initial_configure_sent = with_states(surface, |states| {
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
                            let configured = with_states(surface, |states| {
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
                                let pending = not_mapped_list.remove(&toplevel);

                                if let Some(win) = pending {
                                    let mut space = self.desktop_layout.borrow_mut();
                                    space.active_workspace().map_toplevel(win, true);
                                }
                            }
                        }
                        #[cfg(feature = "xwayland")]
                        Toplevel::X11(_) => {
                            let pending = not_mapped_list.remove(&toplevel);

                            if let Some(win) = pending {
                                let mut space = self.desktop_layout.borrow_mut();
                                space.active_workspace().map_toplevel(win, true);
                            }
                        }
                    }
                }
            }
        }

        // Update maped windows
        {
            // In visible workspaces
            for workspace in self.desktop_layout.borrow_mut().visible_workspaces() {
                let mut window_map = workspace.windows_mut();
                if let Some(window) = window_map.find_mut(surface) {
                    window.self_update();

                    let geometry = window.geometry();
                    let new_location = with_states(surface, |states| {
                        let mut data = states
                            .data_map
                            .get::<RefCell<SurfaceData>>()
                            .unwrap()
                            .borrow_mut();

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
                        match data.move_after_resize_state {
                            MoveAfterResizeState::WaitingForCommit(mdata) => {
                                new_location = Some(mdata.target_window_location);
                                data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
                            }
                            _ => {}
                        }

                        new_location
                    })
                    .unwrap();

                    if let Some(location) = new_location {
                        window.set_location(location);
                    }
                }
            }

            // Update currently grabed window
            if let Some(grab) = self.desktop_layout.borrow().grabed_window.as_ref() {
                if let Some(s) = grab.window.toplevel().get_surface() {
                    if s == surface {
                        with_states(surface, |states| {
                            let mut data = states
                                .data_map
                                .get::<RefCell<SurfaceData>>()
                                .unwrap()
                                .borrow_mut();

                            // If the compositor requested MoveAfterReszie
                            if let MoveAfterResizeState::WaitingForCommit(mdata) =
                                data.move_after_resize_state
                            {
                                data.move_after_resize_state = MoveAfterResizeState::Current(mdata);
                            }
                        })
                        .unwrap();
                    }
                }
            }
        }

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

        // TODO:
        // let mut window_map = self.window_map.borrow_mut();
        // if let Some(layer) = window_map.layers.find(surface) {
        //     // send the initial configure if relevant
        //     let initial_configure_sent = with_states(surface, |states| {
        //         states
        //             .data_map
        //             .get::<Mutex<LayerSurfaceAttributes>>()
        //             .unwrap()
        //             .lock()
        //             .unwrap()
        //             .initial_configure_sent
        //     })
        //     .unwrap();
        //     if !initial_configure_sent {
        //         layer.surface.send_configure();
        //     }

        //     if let Some(output) = self.output_map.borrow().find_by_layer_surface(surface) {
        //         window_map.layers.arange_layers(output);
        //     }
        // }
    }
}

pub fn init_shell<BackendData: 'static>(display: Rc<RefCell<Display>>, log: ::slog::Logger) {
    // Create the compositor
    compositor_init(
        &mut *display.borrow_mut(),
        move |surface, mut ddata| {
            let state = ddata.get::<BackendState<BackendData>>().unwrap();
            state.anodium.surface_commit(&surface);
        },
        log.clone(),
    );

    // init the xdg_shell
    xdg_shell_init(
        &mut *display.borrow_mut(),
        move |request, mut dispatch_data| {
            let state = dispatch_data.get::<BackendState<BackendData>>().unwrap();
            state.anodium.xdg_shell_request(request);
        },
        log.clone(),
    );

    wlr_layer_shell_init(
        &mut *display.borrow_mut(),
        move |request, mut ddata| {
            let state = ddata.get::<BackendState<BackendData>>().unwrap();
            state.anodium.wlr_layer_shell_request(request);
        },
        log.clone(),
    );
}

// fn fullscreen_output_geometry(
//     wl_surface: &wl_surface::WlSurface,
//     wl_output: Option<&wl_output::WlOutput>,
//     window_map: &WindowMap,
//     output_map: &OutputMap,
// ) -> Option<Rectangle<i32, Logical>> {
//     // First test if a specific output has been requested
//     // if the requested output is not found ignore the request
//     if let Some(wl_output) = wl_output {
//         return output_map.find_by_output(&wl_output).map(|o| o.geometry());
//     }

//     // There is no output preference, try to find the output
//     // where the window is currently active
//     let window_location = window_map
//         .windows()
//         .find(wl_surface)
//         .map(|window| window.location());

//     if let Some(location) = window_location {
//         let window_output = output_map.find_by_position(location).map(|o| o.geometry());

//         if let Some(result) = window_output {
//             return Some(result);
//         }
//     }

//     // Fallback to primary output
//     output_map.with_primary().map(|o| o.geometry())
// }
