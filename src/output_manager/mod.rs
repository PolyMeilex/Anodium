mod output;
pub use output::{Output, OutputDescriptor};

use crate::workspace::Workspace;

pub use smithay::wayland::output::Output as SmithayOutput;
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

#[derive(Debug, Clone)]
pub struct OutputManager {
    outputs: Rc<RefCell<Vec<Output>>>,
    // space: desktop::Space,
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
            // space: desktop::Space::new(slog_scope::logger()),
        }
    }

    pub fn outputs(&self) -> Ref<Vec<Output>> {
        self.outputs.borrow()
    }

    pub fn add(&mut self, space: &mut Workspace, output: &Output) {
        let loc = (
            space
                .outputs()
                .fold(0, |acc, o| acc + space.output_geometry(o).unwrap().size.w),
            0,
        );

        output.change_current_state(None, None, None, Some(loc.into()));
        space.map_output(output, 1.0, loc);
        self.outputs.borrow_mut().push(output.clone());
    }
}
