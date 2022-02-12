use std::cell::{Ref, RefCell};
use std::rc::Rc;

mod region;
mod workspace;

use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::output::Output;

use crate::utils::iterators::RefIter;

pub use self::region::Region;
pub use self::workspace::Workspace;

#[derive(Debug, Clone, Default)]
pub struct RegionManager {
    regions: Rc<RefCell<Vec<Region>>>,
}

impl RegionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, region: Region) {
        self.regions.borrow_mut().push(region)
    }

    pub fn first(&self) -> Option<Region> {
        self.regions.borrow().first().cloned()
    }

    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        for region in self.regions.borrow().iter() {
            if let Some((surface, point)) = region.surface_under(point) {
                return Some((surface, point));
            }
        }
        None
    }

    pub fn region_under(&self, point: Point<f64, Logical>) -> Option<Region> {
        for region in self.regions.borrow().iter() {
            if region.contains(point) {
                return Some(region.clone());
            }
        }
        None
    }

    pub fn find_window_region(&self, window: &Window) -> Option<Region> {
        for region in self.regions.borrow().iter() {
            if region.find_window_workspace(window).is_some() {
                return Some(region.clone());
            }
        }
        None
    }

    pub fn find_window_workspace(&self, window: &Window) -> Option<Workspace> {
        for region in self.regions.borrow().iter() {
            if let Some(workspace) = region.find_window_workspace(window) {
                return Some(workspace);
            }
        }
        None
    }

    pub fn find_surface_workspace(&self, surface: &WlSurface) -> Option<Workspace> {
        for region in self.regions.borrow().iter() {
            if let Some(workspace) = region.find_surface_workspace(surface) {
                return Some(workspace);
            }
        }
        None
    }

    pub fn find_output_region(&self, output: &Output) -> Option<Region> {
        for region in self.regions.borrow().iter() {
            if region.has_output(output) {
                return Some(region.clone());
            }
        }
        None
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        for region in self.regions.borrow().iter() {
            if let Some(window) = region.window_for_surface(surface) {
                return Some(window);
            }
        }
        None
    }

    pub fn send_frames(&self, all: bool, time: u32) {
        for region in self.regions.borrow().iter() {
            region
                .active_workspace()
                .unwrap()
                .space()
                .send_frames(all, time);
        }
    }

    pub fn iter(&self) -> RefIter<Region> {
        RefIter {
            inner: Some(Ref::map(self.regions.borrow(), |v| &v[..])),
        }
    }

    pub fn refresh(&self) {
        for region in self.regions.borrow().iter() {
            region.active_workspace().unwrap().space_mut().refresh();
        }
    }
}
