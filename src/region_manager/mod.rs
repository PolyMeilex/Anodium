use std::cell::RefCell;
use std::rc::Rc;

mod region;
mod workspace;

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
}
