use core::slice;

pub use super::{Window, WindowSurface};

#[derive(Default, Debug)]
pub struct WindowList {
    pub windows: Vec<Window>,
}

impl WindowList {}
