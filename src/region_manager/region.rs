use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexSet;

use smithay::utils::{Logical, Physical, Point};

use crate::output_manager::Output;

use super::workspace::Workspace;

#[derive(Debug)]
struct RegionInner {
    position: Point<i32, Physical>,
    active_workspace: Option<Workspace>,
    workspaces: IndexSet<Workspace>,
}

#[derive(Debug, Clone)]
pub struct Region {
    inner: Rc<RefCell<RegionInner>>,
}

impl Region {
    pub fn new(position: Point<i32, Physical>) -> Self {
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
}
