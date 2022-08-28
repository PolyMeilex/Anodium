use crate::State;
use smithay::delegate_output;

mod backend_handler;
mod input_handler;
mod output_handler;

mod compositor;
mod data_device_handler;
mod dmabuf;
mod seat_handler;
mod xdg_shell;

//
// Wl Output & Xdg Output
//

delegate_output!(State);
