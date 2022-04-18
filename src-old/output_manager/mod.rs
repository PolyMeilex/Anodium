mod output;
pub use output::{Output, OutputDescriptor};

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
        self.outputs.borrow_mut().push(output.clone());
    }

    pub fn into_iter(self) -> IntoIter<Output> {
        self.outputs.borrow().clone().into_iter()
    }
}