use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::output::Mode;
use smithay::wayland::shell::wlr_layer::{self, Layer};
use smithay::{reexports::wayland_server::Display, wayland::output::PhysicalProperties};

mod output_map;
pub use output_map::{Output, OutputMap};

mod iterators;
use iterators::{VisibleWorkspaceIter, VisibleWorkspaceIterMut};

use crate::config::ConfigVM;
// use crate::positioner::floating::Floating as Universal;
use crate::positioner::universal::Universal;
use crate::positioner::Positioner;
use crate::utils::AsWlSurface;

pub mod popup;
pub use popup::{Popup, PopupKind, PopupList};

pub mod window;
pub use window::{Toplevel, Window, WindowList};

mod layer_map;

#[derive(Debug)]
pub struct DesktopLayout {
    pub output_map: OutputMap,

    pub workspaces: HashMap<String, Box<dyn Positioner>>,
    active_workspace: Option<String>,

    pub grabed_window: Option<Window>,

    pointer_location: Point<f64, Logical>,
}

impl DesktopLayout {
    pub fn new(display: Rc<RefCell<Display>>, config: ConfigVM, log: slog::Logger) -> Self {
        Self {
            output_map: OutputMap::new(display, config, log),

            workspaces: Default::default(),
            active_workspace: None,

            grabed_window: Default::default(),
            pointer_location: Default::default(),
        }
    }

    pub fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {
        self.pointer_location = pos;

        for (id, w) in self.workspaces.iter_mut() {
            w.on_pointer_move(pos);

            if w.geometry().contains(pos.to_i32_round()) {
                self.active_workspace = Some(id.clone());
            }
        }
    }

    pub fn send_frames(&self, time: u32) {
        for w in self.visible_workspaces() {
            w.send_frames(time);
        }
        self.output_map.send_frames(time);
    }

    pub fn surface_under(&self, point: Point<f64, Logical>) -> Option<(WlSurface, Point<i32, Logical>)> {
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

    // pub fn bring_surface_to_top<S: AsWlSurface>(&mut self, surface: &S) {
    //     for w in self.visible_workspaces() {
    //         w.windows_mut().bring_surface_to_top(surface);
    //     }
    // }

    pub fn update(&mut self, delta: f64) {
        for (_, w) in self.workspaces.iter_mut() {
            w.update(delta);
        }

        self.output_map.refresh();
    }
}

// Workspaces
impl DesktopLayout {
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
    pub fn find_workspace_by_surface<S: AsWlSurface>(&self, surface: &S) -> Option<&dyn Positioner> {
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
            if let Some(w) = self.workspaces.get_mut(key) {
                w.set_geometry(output.usable_geometry());
            }
        }
    }

    pub fn switch_workspace(&mut self, key: &str) {
        for o in self.output_map.iter_mut() {
            if o.geometry().to_f64().contains(self.pointer_location) {
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

// Outputs
impl DesktopLayout {
    pub fn add_output<N, CB>(
        &mut self,
        name: N,
        physical: PhysicalProperties,
        mode: smithay::wayland::output::Mode,
        after: CB,
    ) where
        N: AsRef<str>,
        CB: FnOnce(&Output),
    {
        let id = self.workspaces.len() + 1;
        let id = format!("{}", id);

        if self.active_workspace.is_none() {
            self.active_workspace = Some(id.clone());
        }

        let output = self.output_map.add(name, physical, mode, id.clone());
        after(output);

        let mut positioner = Universal::new(Default::default(), Default::default());
        positioner.set_geometry(output.usable_geometry());

        self.workspaces.insert(id.into(), Box::new(positioner));
    }

    pub fn update_output_mode_by_name<N: AsRef<str>>(&mut self, mode: Mode, name: N) {
        let name = name.as_ref();
        self.output_map.update_by_name(Some(mode), None, name);

        let output = self.output_map.find_by_name(name).unwrap();
        let space = self.workspaces.get_mut(output.active_workspace()).unwrap();
        space.set_geometry(output.usable_geometry());
    }

    pub fn retain_outputs<F>(&mut self, f: F)
    where
        F: FnMut(&Output) -> bool,
    {
        self.output_map.retain(f);
    }
}

impl DesktopLayout {
    pub fn arrange_layers(&mut self) {
        self.output_map.arrange_layers();
        self.update_workspaces_geometry();
    }

    pub fn insert_layer(
        &mut self,
        output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        surface: wlr_layer::LayerSurface,
        layer: wlr_layer::Layer,
    ) {
        self.output_map.insert_layer(output, surface, layer);
        self.update_workspaces_geometry();
    }
}
