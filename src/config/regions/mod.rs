use std::cell::RefCell;
use std::rc::Rc;

use rhai::{plugin::*, Array, AST};
use rhai::{FnPtr, INT};

use crate::region_manager::{Region, RegionManager, Workspace};

use crate::output_manager::Output;

mod workspace;

use smithay::utils::{Logical, Physical, Point};

#[derive(Debug, Clone)]
pub struct Regions {
    regions_map: RegionManager,
}

impl Regions {
    pub fn new(regions_map: RegionManager) -> Self {
        Self { regions_map }
    }
}

impl IntoIterator for Regions {
    type Item = Region;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.regions_map.into_iter()
    }
}

#[export_module]
pub mod region {
    pub fn create(position: Point<i32, Logical>) -> Region {
        Region::new(position)
    }

    #[rhai_fn(global)]
    pub fn add_workspace(region: &mut Region, workspace: Workspace) {
        region.add_workspace(workspace);
    }

    #[rhai_fn(global)]
    pub fn map_output(
        region: &mut Region,
        output: Output,
        scale: f64,
        position: Point<i32, Logical>,
    ) {
        region.map_output(&output, scale, position);
    }

    #[rhai_fn(set = "position", pure)]
    pub fn set_position(region: &mut Region, position: Point<i32, Logical>) {
        region.set_position(position);
    }

    #[rhai_fn(get = "position", pure)]
    pub fn get_position(region: &mut Region) -> Point<i32, Logical> {
        region.position()
    }
}

#[export_module]
pub mod regions {
    #[rhai_fn(global)]
    pub fn push(regions: &mut Regions, region: Region) {
        regions.regions_map.push(region)
    }
}

#[export_module]
pub mod point {
    pub fn physical(x: INT, y: INT) -> Point<i32, Physical> {
        Point::from((x as i32, y as i32))
    }

    pub fn logical(x: INT, y: INT) -> Point<i32, Logical> {
        Point::from((x as i32, y as i32))
    }
}

pub fn register(engine: &mut Engine) {
    let region_module = exported_module!(region);
    let regions_module = exported_module!(regions);
    let point_module = exported_module!(point);
    engine
        .register_static_module("regions", regions_module.into())
        .register_static_module("region", region_module.into())
        .register_static_module("point", point_module.into())
        .register_type::<Point<i32, Physical>>()
        .register_type::<Region>()
        .register_iterator::<Regions>();

    workspace::register(engine);
}
