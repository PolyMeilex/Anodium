use rhai::plugin::*;
use std::process::Command; // a "prelude" import for macros

#[derive(Debug, Clone)]
pub struct System;

impl System {
    pub fn new() -> Self {
        Self
    }
}

#[export_module]
pub mod system {
    #[rhai_fn(global)]
    pub fn exec(_system: &mut System, command: &str) {
        if let Err(e) = Command::new(command).spawn() {
            slog_scope::error!("failed to start command: {}, err: {:?}", command, e);
        }
    }
}

pub fn register(engine: &mut Engine) {
    let system_module = exported_module!(system);

    engine
        .register_static_module("system", system_module.into())
        .register_type::<System>();
}
