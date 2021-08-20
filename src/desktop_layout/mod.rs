use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::output::Mode;
use smithay::{reexports::wayland_server::Display, wayland::output::PhysicalProperties};

mod output_map;
pub use output_map::{Output, OutputMap};

mod iterators;
use iterators::{VisibleWorkspaceIter, VisibleWorkspaceIterMut};

use crate::positioner::floating::Floating as Universal;
// use crate::positioner::universal::Universal;
use crate::positioner::Positioner;
use crate::utils::AsWlSurface;

pub mod popup;
pub use popup::{Popup, PopupKind, PopupList};

pub mod window;
pub use window::{Toplevel, Window, WindowList};

#[derive(Debug)]
pub struct GrabState {
    pub window: Window,
    pub done: bool,
}

#[derive(Debug)]
pub struct DesktopLayout {
    pub output_map: OutputMap,

    workspaces: HashMap<String, Box<dyn Positioner>>,
    active_workspaces: Option<String>,

    pub grabed_window: Rc<RefCell<Option<GrabState>>>,
}

impl DesktopLayout {
    pub fn new(display: Rc<RefCell<Display>>, log: slog::Logger) -> Self {
        Self {
            output_map: OutputMap::new(display, log),

            workspaces: Default::default(),
            active_workspaces: None,

            grabed_window: Default::default(),
        }
    }

    pub fn on_pointer_move(&mut self, pos: Point<f64, Logical>) {
        for (id, w) in self.workspaces.iter_mut() {
            w.on_pointer_move(pos);

            if w.geometry().contains(pos.to_i32_round()) {
                self.active_workspaces = Some(id.clone());
            }
        }
    }

    pub fn send_frames(&self, time: u32) {
        for w in self.visible_workspaces() {
            w.send_frames(time);
        }
    }

    pub fn surface_under(&self, point: Point<f64, Logical>) -> Option<(WlSurface, Point<i32, Logical>)> {
        for w in self.visible_workspaces() {
            let under = w.windows().surface_under(point);
            if under.is_some() {
                return under;
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

        if self
            .grabed_window
            .borrow()
            .as_ref()
            .map(|s| s.done)
            .unwrap_or(false)
        {
            let state = self.grabed_window.borrow_mut().take().unwrap();

            let location = state.window.location() + state.window.geometry().loc;

            if let Some(key) = self
                .output_map
                .find_by_position(location)
                .map(|o| o.active_workspace())
            {
                self.workspaces
                    .get_mut(key)
                    .unwrap()
                    .map_toplevel(state.window, false);
            } else {
                self.active_workspace().map_toplevel(state.window, false);
            }
        }
    }
}

// Workspaces
impl DesktopLayout {
    pub fn active_workspace(&mut self) -> &mut dyn Positioner {
        self.workspaces
            .get_mut(self.active_workspaces.as_ref().unwrap())
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
            if w.windows().find(surface).is_some() {
                return Some(w);
            }
        }
        None
    }

    pub fn find_workspace_by_surface_mut<S: AsWlSurface>(
        &mut self,
        surface: &S,
    ) -> Option<&mut dyn Positioner> {
        for w in self.visible_workspaces_mut() {
            if w.windows().find(surface).is_some() {
                return Some(w);
            }
        }
        None
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

        if self.active_workspaces.is_none() {
            self.active_workspaces = Some(id.clone());
        }

        let output = self.output_map.add(name, physical, mode, id.clone());
        after(output);

        let mut positioner = Universal::new(Default::default(), Default::default());
        positioner.set_geometry(output.geometry());

        self.workspaces.insert(id.into(), Box::new(positioner));
    }

    pub fn update_output_mode_by_name<N: AsRef<str>>(&mut self, mode: Mode, name: N) {
        let name = name.as_ref();
        self.output_map.update_by_name(Some(mode), None, name);

        let output = self.output_map.find_by_name(name).unwrap();
        let space = self.workspaces.get_mut(output.active_workspace()).unwrap();
        space.set_geometry(output.geometry());
    }

    pub fn retain_outputs<F>(&mut self, f: F)
    where
        F: FnMut(&Output) -> bool,
    {
        self.output_map.retain(f);
    }
}
