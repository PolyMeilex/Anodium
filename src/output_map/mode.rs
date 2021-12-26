/*use std::convert::From;

use smithay::wayland::output::Mode as WaylandMode;
use smithay::{reexports::drm::control::Mode as DrmMode, utils::Size};

impl From<DrmMode> for WaylandMode {
    fn from(mode: DrmMode) -> Self {
        Self {
            size: Size::from(0, 0),
            refresh: 0,
        }
    }
}*/
