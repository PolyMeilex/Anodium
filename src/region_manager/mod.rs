use std::cell::RefCell;
use std::rc::Rc;

mod region;
mod workspace;

use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};

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
            return Some(region.clone());
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

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        for region in self.regions.borrow().iter() {
            if let Some(window) = region.window_for_surface(surface) {
                return Some(window);
            }
        }
        None
    }
}
