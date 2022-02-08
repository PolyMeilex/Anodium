use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexSet;

use smithay::{
    desktop::{Window, WindowSurfaceType},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point},
};

use crate::output_manager::Output;

use super::workspace::Workspace;

#[derive(Debug)]
struct RegionInner {
    position: Point<i32, Logical>,
    active_workspace: Option<Workspace>,
    workspaces: IndexSet<Workspace>,
}

#[derive(Debug, Clone)]
pub struct Region {
    inner: Rc<RefCell<RegionInner>>,
}

impl Region {
    pub fn new(position: Point<i32, Logical>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(RegionInner {
                position,
                active_workspace: None,
                workspaces: IndexSet::new(),
            })),
        }
    }

    pub fn add_workspace(&self, workspace: Workspace) {
        let mut inner = self.inner.borrow_mut();
        inner.workspaces.insert(workspace.clone());
        inner.active_workspace = Some(workspace);
    }

    pub fn map_output(&self, output: &Output, scale: f64, location: Point<i32, Logical>) {
        for workspace in &self.inner.borrow().workspaces {
            workspace.space_mut().map_output(output, scale, location);
        }
    }

    pub fn set_active_workspace(&self, name: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.active_workspace = inner.workspaces.get(name).cloned()
    }

    pub fn active_workspace(&self) -> Option<Workspace> {
        let inner = self.inner.borrow();
        inner.active_workspace.clone()
    }

    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let inner = self.inner.borrow();
        let active_workspace = inner.active_workspace.as_ref().unwrap();
        let space = active_workspace.space();
        point += inner.position.to_f64();
        let window = space.window_under(point)?;

        let window_loc = space.window_geometry(window).unwrap().loc;
        window
            .surface_under(point - window_loc.to_f64(), WindowSurfaceType::ALL)
            .map(|(s, loc)| (s, loc + window_loc))
    }

    pub fn contains(&self, point: Point<f64, Logical>) -> bool {
        let inner = self.inner.borrow();
        let active_workspace = inner.active_workspace.as_ref().unwrap();
        let space = active_workspace.space();

        for output in space.outputs() {
            let mut geometry = space.output_geometry(output).unwrap();
            geometry.loc += inner.position;
            if geometry.to_f64().contains(point) {
                return true;
            }
        }

        false
    }

    pub fn map_window<P: Into<Point<i32, Logical>>>(
        &self,
        window: &Window,
        location: P,
        activate: bool,
    ) {
        self.active_workspace()
            .unwrap()
            .space_mut()
            .map_window(window, location, activate);
    }
}
