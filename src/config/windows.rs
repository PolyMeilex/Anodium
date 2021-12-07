use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult};

use smithay::reexports::calloop::channel::Sender;

use super::eventloop::ConfigEvent;

#[derive(Debug, Clone)]
pub struct Windows {
    event_sender: Sender<ConfigEvent>,
}

impl Windows {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self { event_sender }
    }
}

#[export_module]
pub mod windows {}

pub fn register(engine: &mut Engine) {
    let windows_module = exported_module!(windows);

    engine
        .register_static_module("windows", windows_module.into())
        .register_type::<Windows>();
}
