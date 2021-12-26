use crate::window::Window;

#[derive(Debug)]
pub enum ConfigEvent {
    SwitchWorkspace(String),
    Close(Window),
    Maximize(Window),
    Unmaximize(Window),
    OutputsRearrange,
}
