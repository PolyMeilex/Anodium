use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::output::Mode;
use smithay::wayland::shell::wlr_layer::Layer;

mod output_map;
pub use output_map::{Output, OutputMap};

mod iterators;
use iterators::{VisibleWorkspaceIter, VisibleWorkspaceIterMut};

// use crate::positioner::floating::Floating as Universal;
use crate::positioner::universal::Universal;
use crate::positioner::Positioner;
use crate::state::Anodium;
use crate::utils::AsWlSurface;

pub mod popup;
pub use popup::{Popup, PopupKind, PopupList};

pub mod window;
pub use window::{Window, WindowList, WindowSurface};

mod layer_map;
pub use layer_map::LayerSurface;

impl Anodium {
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

// Workspaces
impl Anodium {
    pub fn active_workspace(&mut self) -> &mut dyn Positioner {
        self.workspaces
            .get_mut(self.active_workspace.as_ref().unwrap())
            .unwrap()
            .as_mut()
    }

    pub fn visible_workspaces(&self) -> impl Iterator<Item = &dyn Positioner> {
        VisibleWorkspaceIter::new(&self.output_map, &self.workspaces)
    }

    pub fn visible_workspaces_mut(&mut self) -> impl Iterator<Item = &mut dyn Positioner> {
        VisibleWorkspaceIterMut::new(&self.output_map, &mut self.workspaces)
    }

    #[allow(dead_code)]
    pub fn find_workspace_by_surface<S: AsWlSurface>(
        &self,
        surface: &S,
    ) -> Option<&dyn Positioner> {
        for w in self.visible_workspaces() {
            if let Some(surface) = surface.as_surface() {
                if w.find_window(surface).is_some() {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn find_workspace_by_surface_mut<S: AsWlSurface>(
        &mut self,
        surface: &S,
    ) -> Option<&mut dyn Positioner> {
        for w in self.visible_workspaces_mut() {
            if let Some(surface) = surface.as_surface() {
                if w.find_window(surface).is_some() {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn update_workspaces_geometry(&mut self) {
        for output in self.output_map.iter() {
            let key = output.active_workspace();
            if let Some(w) = self.workspaces.get_mut(&key) {
                w.set_geometry(output.usable_geometry());
            }
        }
    }

    pub fn switch_workspace(&mut self, key: &str) {
        let already_active = self.output_map.iter().any(|o| &o.active_workspace() == key);

        if already_active {
            if let Some(workspace) = self.workspaces.get(key) {
                let geometry = workspace.geometry();
                let loc = geometry.loc;
                let size = geometry.size;

                self.input_state.pointer_location.x = (loc.x + size.w / 2) as f64;
                self.input_state.pointer_location.y = (loc.y + size.h / 2) as f64;
            }
        } else {
            for o in self.output_map.iter_mut() {
                if o.geometry()
                    .to_f64()
                    .contains(self.input_state.pointer_location)
                {
                    if self.workspaces.get(key).is_none() {
                        let positioner = Universal::new(Default::default(), Default::default());
                        self.workspaces.insert(key.into(), Box::new(positioner));
                    }
                    o.set_active_workspace(key.into());
                    break;
                }
            }

            self.active_workspace = Some(key.into());
            self.update_workspaces_geometry();
        }
    }
}

// Outputs
impl Anodium {
    pub fn add_output(&mut self, mut output: Output) {
        let id = self.workspaces.len() + 1;
        let id = format!("{}", id);

        if self.active_workspace.is_none() {
            self.active_workspace = Some(id.clone());
        }

        output.set_active_workspace(id.clone());
        self.output_map.add(output);

        let positioner = Universal::new(Default::default(), Default::default());

        self.workspaces.insert(id, Box::new(positioner));
        self.update_workspaces_geometry();
    }

    pub fn update_output_mode_by_name<N: AsRef<str>>(&mut self, mode: Mode, name: N) {
        let name = name.as_ref();
        self.output_map.update_by_name(Some(mode), None, name);

        let output = self.output_map.find_by_name(name).unwrap();
        let space = self.workspaces.get_mut(&output.active_workspace()).unwrap();
        space.set_geometry(output.usable_geometry());
    }

    pub fn arrange_wlr_layers(&mut self) {
        self.output_map.arrange_layers();
        self.update_workspaces_geometry();
    }

    pub fn insert_wlr_layer(
        &mut self,
        output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        layer: LayerSurface,
    ) {
        self.output_map.insert_layer(output, layer);
        self.update_workspaces_geometry();
    }
}
