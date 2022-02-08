use smithay::desktop;

use crate::output_manager::Output;

#[derive(Debug, PartialEq)]
pub struct Workspace {
    space: desktop::Space,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        let space = desktop::Space::new(slog_scope::logger());
        Self { space }
    }

    pub fn output(&self) -> Option<Output> {
        self.space.outputs().next().cloned().map(Output::wrap)
    }
}

impl std::ops::Deref for Workspace {
    type Target = desktop::Space;

    fn deref(&self) -> &Self::Target {
        &self.space
    }
}

impl std::ops::DerefMut for Workspace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.space
    }
}
