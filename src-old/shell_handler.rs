use smithay::{
    desktop::{self},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

use crate::{
    framework::{
        shell::{ShellEvent, ShellHandler},
        surface_data::{ResizeData, ResizeState, SurfaceData},
    },
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    output_manager::Output,
    state::Anodium,
};

impl ShellHandler for Anodium {
    fn on_shell_event(&mut self, event: ShellEvent) {
        match event {
            //
            // Toplevel
            //
            ShellEvent::WindowCreated { window } => {
                let pos = window
                    .user_data()
                    .get::<anodium_framework::shell::X11WindowUserData>()
                    .map(|i| i.location)
                    .unwrap_or_default();

                self.region_manager
                    .region_under(self.input_state.borrow().pointer_location)
                    .unwrap_or_else(|| self.region_manager.first().unwrap())
                    .active_workspace()
                    .space_mut()
                    .map_window(&window, pos, false);
            }

            ShellEvent::WindowMove {
                window,
                start_data,
                seat,
                serial,
            } => {
                let pointer = seat.get_pointer().unwrap();

                let workspace = self.region_manager.find_window_workspace(&window).unwrap();
                let initial_window_location = workspace.space().window_location(&window).unwrap();

                let grab = MoveSurfaceGrab {
                    start_data,
                    window,
                    initial_window_location,
                };
                pointer.set_grab(grab, serial, 0);
            }

            ShellEvent::WindowResize {
                window,
                start_data,
                seat,
                edges,
                serial,
            } => {
                let pointer = seat.get_pointer().unwrap();
                let wl_surface = window.toplevel().get_surface();

                if let Some(wl_surface) = wl_surface {
                    let region = self.region_manager.find_window_region(&window).unwrap();
                    let workspace = region.find_window_workspace(&window).unwrap();
                    let loc = workspace.space().window_location(&window).unwrap();
                    let geometry = window.geometry();

                    let (initial_window_location, initial_window_size) =
                        (loc + region.position(), geometry.size);

                    SurfaceData::with_mut(wl_surface, |data| {
                        data.resize_state = ResizeState::Resizing(ResizeData {
                            edges,
                            initial_window_location,
                            initial_window_size,
                        });
                    });

                    let grab = ResizeSurfaceGrab {
                        start_data,
                        window,
                        edges,
                        initial_window_size,
                        last_window_size: initial_window_size,
                    };

                    pointer.set_grab(grab, serial, 0);
                }
            }

            ShellEvent::WindowGotResized {
                window,
                new_location_x,
                new_location_y,
            } => {
                if let Some(region) = self.region_manager.find_window_region(&window) {
                    let space = region.find_window_workspace(&window).unwrap();

                    let mut new_location =
                        space.space().window_location(&window).unwrap_or_default();

                    if let Some(x) = new_location_x {
                        new_location.x = x;
                    }

                    if let Some(y) = new_location_y {
                        new_location.y = y;
                    }

                    let new_location = new_location - region.position();

                    region
                        .find_window_workspace(&window)
                        .unwrap()
                        .space_mut()
                        .map_window(&window, new_location, false);
                }
            }

            ShellEvent::WindowMaximize { .. } => {}
            ShellEvent::WindowUnMaximize { .. } => {}

            //
            // Popup
            //
            ShellEvent::PopupCreated { .. } => {}
            ShellEvent::PopupGrab { .. } => {}

            //
            // Wlr Layer Shell
            //
            ShellEvent::LayerCreated {
                surface, output, ..
            } => {
                let output = output
                    .and_then(|o| Output::from_resource(&o))
                    .unwrap_or_else(|| {
                        Output::wrap(
                            self.region_manager
                                .first()
                                .unwrap()
                                .active_workspace()
                                .space()
                                .outputs()
                                .next()
                                .unwrap()
                                .clone(),
                        )
                    });

                let mut map = output.layer_map();
                map.map_layer(&surface).unwrap();
            }
            ShellEvent::LayerAckConfigure { surface, .. } => {
                if let Some(output) = self
                    .region_manager
                    .find_surface_workspace(&surface)
                    .unwrap()
                    .space()
                    .outputs()
                    .find(|o| {
                        let map = desktop::layer_map_for_output(o);
                        map.layer_for_surface(&surface).is_some()
                    })
                {
                    let mut map = desktop::layer_map_for_output(output);
                    map.arrange();
                }
            }

            ShellEvent::SurfaceCommit { surface } => {
                if let Some(workspace) = self.region_manager.find_surface_workspace(&surface) {
                    workspace.space().commit(&surface);
                }
            }
            _ => {}
        }
    }
}

impl Anodium {
    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        self.region_manager.surface_under(point)
    }
}
