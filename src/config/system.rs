use rhai::plugin::*;
use std::process::Command; // a "prelude" import for macros

#[export_module]
pub mod exports {
    pub fn exec(command: &str) {
        if let Err(e) = Command::new(command).spawn() {
            slog_scope::error!("failed to start command: {}, err: {:?}", command, e);
        }
    }
}

pub fn register(engine: &mut Engine) {
    let exports_module = exported_module!(exports);
    engine.register_static_module("system", exports_module.into());
}
