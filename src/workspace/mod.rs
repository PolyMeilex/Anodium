use smithay::desktop;

#[derive(PartialEq)]
pub struct Workspace {
    space: desktop::Space,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            space: desktop::Space::new(slog_scope::logger()),
        }
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
