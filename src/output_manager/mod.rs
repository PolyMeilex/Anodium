mod output;
pub use output::{Output, OutputDescriptor};
use smithay::{
    desktop::WindowSurfaceType,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
};

use crate::workspace::Workspace;

pub use smithay::wayland::output::Output as SmithayOutput;
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
    vec::IntoIter,
};

#[derive(Debug, Clone)]
pub struct OutputManager {
    outputs: Rc<RefCell<Vec<Output>>>,
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: Default::default(),
        }
    }

    pub fn outputs(&self) -> Ref<Vec<Output>> {
        self.outputs.borrow()
    }

    pub fn add(&mut self, output: &Output) {
        output.change_current_state(None, None, None, Some((0, 0).into()));
        self.outputs.borrow_mut().push(output.clone());
    }

    pub fn into_iter(&self) -> IntoIter<Output> {
        self.outputs.borrow().clone().into_iter()
    }
}
