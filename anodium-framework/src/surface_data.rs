use smithay::utils::{Logical, Point, Size};

/// Information about the fullscrean operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MoveAfterResizeData {
    /// The initial window location.
    pub initial_window_location: Point<i32, Logical>,
    /// The initial window geometry.
    pub initial_size: Size<i32, Logical>,

    /// The target window location.
    pub target_window_location: Point<i32, Logical>,
    /// The target window geometry.
    pub target_size: Size<i32, Logical>,
}

/// State of the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[allow(dead_code)]
pub enum MoveAfterResizeState {
    /// Idle
    None,
    /// The surface was resized and moved
    Current(MoveAfterResizeData),
    /// Waiting for commit
    WaitingForCommit(MoveAfterResizeData),
}

impl Default for MoveAfterResizeState {
    fn default() -> Self {
        Self::None
    }
}
