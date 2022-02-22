use rhai::FnPtr;

use crate::output_manager::Output;
use smithay::desktop::Window;

use smithay::wayland::output::Mode;

#[derive(Debug)]
pub enum ConfigEvent {
    SwitchWorkspace(String),
    Close(Window),
    Maximize(Window),
    Unmaximize(Window),
    OutputsRearrange,
    OutputUpdateMode(Output, Mode),
    Shell(FnPtr),
}
