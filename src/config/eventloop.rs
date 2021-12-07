use rhai::{plugin::*, Scope};
use rhai::{Dynamic, EvalAltResult, FnPtr};

use smithay::reexports::calloop::channel::Sender;

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

#[derive(Debug, Clone)]
pub struct EventLoop(pub Sender<ConfigEvent>);

impl EventLoop {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self(event_sender)
    }
}
