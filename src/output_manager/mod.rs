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

    pub fn add(&mut self, output: &Output) {
        output.change_current_state(None, None, None, Some((0, 0).into()));
        self.outputs.borrow_mut().push(output.clone());
    }
    /*pub fn output_under(&self, point: Point<f64, Logical>) -> Output {
        let sorted_outputs = self.sorted_outputs();
        let mut width = 0.0;
        for output in sorted_outputs {
            let size = output.logical_size();
            if Rectangle::from_loc_and_size(Point::from((0.0, width)), size).contains(point) {
                return output;
            }
            width += size.w;
        }
        sorted_outputs.last().unwrap().clone()
    }

    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let output_under = self.output_under(point);
        let active_workspace = output_under.active_workspace();
        let window = active_workspace.window_under(point)?;

        let window_loc = active_workspace.window_geometry(window).unwrap().loc;
        window
            .surface_under(point - window_loc.to_f64(), WindowSurfaceType::ALL)
            .map(|(s, loc)| (s, loc + window_loc))
    }*/
}
