use crate::window::Window;
use rhai::FnPtr;

#[derive(Debug)]
pub enum ConfigEvent {
    SwitchWorkspace(String),
    Timeout(FnPtr, u64),
    Close(Window),
    Maximize(Window),
    Unmaximize(Window),
    OutputsRearrange,
}
