use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexSet;

use smithay::utils::{Physical, Point};

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
        self.inner.borrow_mut().workspaces.insert(workspace);
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
