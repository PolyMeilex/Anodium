use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::wlr_layer::Layer,
};

use crate::{framework::shell::ShellEvent, grabs::MoveSurfaceGrab, state::Anodium};

impl Anodium {
    pub fn on_shell_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::WindowCreated { window } => {
                self.active_workspace().map_toplevel(window, true);
            }

            ShellEvent::WindowMove {
                toplevel,
                start_data,
                seat,
                serial,
            } => {
                let pointer = seat.get_pointer().unwrap();

                if let Some(space) = self.find_workspace_by_surface_mut(&toplevel) {
                    if let Some(res) = space.move_request(&toplevel, &seat, serial, &start_data) {
                        if let Some(window) = space.unmap_toplevel(&toplevel) {
                            self.grabed_window = Some(window);

                            let grab = MoveSurfaceGrab {
                                start_data,
                                toplevel,
                                initial_window_location: res.initial_window_location,
                            };
                            pointer.set_grab(grab, serial);
                        }
                    }
                }
            }
            ShellEvent::WindowResize {
                toplevel,
                start_data,
                seat,
                edges,
                serial,
            } => {
                if let Some(space) = self.find_workspace_by_surface_mut(&toplevel) {
                    space.resize_request(&toplevel, &seat, serial, start_data, edges);
                }
            }

            ShellEvent::WindowMaximize { toplevel } => {
                if let Some(space) = self.find_workspace_by_surface_mut(&toplevel) {
                    space.maximize_request(&toplevel);
                }
            }
            ShellEvent::WindowUnMaximize { toplevel } => {
                if let Some(space) = self.find_workspace_by_surface_mut(&toplevel) {
                    space.unmaximize_request(&toplevel);
                }
            }

            ShellEvent::LayerCreated {
                surface, output, ..
            } => {
                self.output_map.insert_layer(output, surface);
                self.update_workspaces_geometry();
            }
            ShellEvent::LayerAckConfigure { .. } => {
                self.output_map.arrange_layers();
                self.update_workspaces_geometry();
            }

            ShellEvent::SurfaceCommit { surface } => {
                let found = self
                    .output_map
                    .iter()
                    .any(|o| o.layer_map().find(&surface).is_some());

                if found {
                    self.output_map.arrange_layers();
                    self.update_workspaces_geometry();
                }
            }
            _ => {}
        }
    }

    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        // Layers above windows
        for o in self.output_map.iter() {
            let overlay = o.layer_map().get_surface_under(&Layer::Overlay, point);
            if overlay.is_some() {
                return overlay;
            }
            let top = o.layer_map().get_surface_under(&Layer::Top, point);
            if top.is_some() {
                return top;
            }
        }

        // Windows
        for w in self.visible_workspaces() {
            let under = w.surface_under(point);
            if under.is_some() {
                return under;
            }
        }

        // Layers below windows
        for o in self.output_map.iter() {
            let bottom = o.layer_map().get_surface_under(&Layer::Bottom, point);
            if bottom.is_some() {
                return bottom;
            }
            let background = o.layer_map().get_surface_under(&Layer::Background, point);
            if background.is_some() {
                return background;
            }
        }

        None
    }
}
