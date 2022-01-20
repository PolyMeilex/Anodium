use smithay::desktop;
use smithay::{
    desktop::Kind,
    utils::{Logical, Rectangle},
};

use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    window: smithay::desktop::Window,
}

impl Window {
    pub fn new(toplevel: Kind) -> Self {
        let window = Window {
            window: smithay::desktop::Window::new(toplevel),
        };
        window
    }

    pub fn desktop_window(&self) -> &desktop::Window {
        &self.window
    }
}

impl Window {
    pub fn bbox_in_comp_space(&self, space: &Workspace) -> Rectangle<i32, Logical> {
        space.window_bbox(&self.window).unwrap()
    }

    pub fn bbox_in_window_space(&self) -> Rectangle<i32, Logical> {
        self.window.bbox()
    }
}

impl std::ops::Deref for Window {
    type Target = desktop::Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl std::ops::DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window
    }
}
