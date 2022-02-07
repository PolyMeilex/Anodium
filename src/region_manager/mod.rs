use std::cell::RefCell;
use std::rc::Rc;

mod region;

use self::region::Region;

pub struct RegionManager {
    regions: Rc<RefCell<Vec<Region>>>,
}
