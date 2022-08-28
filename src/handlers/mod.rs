use crate::State;
use smithay::delegate_output;

mod backend;
mod input;
mod output;

mod compositor;
mod data_device;
mod dmabuf;
mod seat;
mod xdg;

//
// Wl Output & Xdg Output
//

delegate_output!(State);
