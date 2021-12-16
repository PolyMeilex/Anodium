use crate::window::Window;
use rhai::FnPtr;

#[derive(Debug)]
pub enum ConfigEvent {
    SwitchWorkspace(String),
    Close(Window),
    Maximize(Window),
    Unmaximize(Window),
    OutputsRearrange,
}
