pub mod move_surface_grab;
pub mod resize_surface_grab;

pub mod surface_data;
pub use surface_data::SurfaceData;
pub use surface_data::{MoveAfterResizeData, MoveAfterResizeState};
use surface_data::{ResizeEdge, ResizeState};

pub mod shell_manager;
