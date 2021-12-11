use crate::window::Window;

use super::FnCallback;

#[derive(Debug)]
pub enum ConfigEvent {
    SwitchWorkspace(String),
    Timeout(FnCallback, u64),
    Close(Window),
    Maximize(Window),
    Unmaximize(Window),
}
