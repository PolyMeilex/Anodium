use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult, FnPtr};

use std::process::Command;

use smithay::reexports::calloop::channel::Sender;

use super::eventloop::ConfigEvent;

#[derive(Debug, Clone)]
pub struct System {
    event_sender: Sender<ConfigEvent>,
}

impl System {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self { event_sender }
    }
}

#[export_module]
pub mod system {
    use crate::config::FnCallback;

    #[rhai_fn(global)]
    pub fn exec(_system: &mut System, command: &str) {
        if let Err(e) = Command::new(command).spawn() {
            slog_scope::error!("failed to start command: {}, err: {:?}", command, e);
        }
    }

    #[rhai_fn(global)]
    pub fn add_timeout(context: NativeCallContext, system: &mut System, fnptr: FnPtr, milis: i64) {
        if milis >= 0 {
            let callback = FnCallback::new(fnptr, context);
            system
                .event_sender
                .send(ConfigEvent::Timeout(callback, milis as u64))
                .unwrap();
        }
    }
}

pub fn register(engine: &mut Engine) {
    let system_module = exported_module!(system);

    engine
        .register_static_module("system", system_module.into())
        .register_type::<System>();
}
