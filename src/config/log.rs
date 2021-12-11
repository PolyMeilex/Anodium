use rhai::plugin::*; // a "prelude" import for macros

#[derive(Debug, Clone)]
pub struct Log;

impl Log {
    pub fn new() -> Self {
        Self
    }
}

#[export_module]
pub mod log {
    #[rhai_fn(global)]
    pub fn info(_log: &mut Log, msg: &str) {
        slog_scope::info!("rhai: {}", msg);
    }

    #[rhai_fn(global)]
    pub fn warn(_log: &mut Log, msg: &str) {
        slog_scope::warn!("rhai: {}", msg);
    }

    #[rhai_fn(global)]
    pub fn error(_log: &mut Log, msg: &str) {
        slog_scope::error!("rhai: {}", msg);
    }
}

pub fn register(engine: &mut Engine) {
    let log_module = exported_module!(log);
    engine
        .register_static_module("log", log_module.into())
        .register_type::<Log>();
}
