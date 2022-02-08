use std::cell::RefCell;
use std::rc::Rc;

mod region;
mod workspace;

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
}
