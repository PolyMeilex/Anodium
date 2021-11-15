use rhai::plugin::*; // a "prelude" import for macros

#[export_module]
pub mod exports {
    pub fn info(msg: &str) {
        slog_scope::info!("rhai: {}", msg);
    }

    pub fn warn(msg: &str) {
        slog_scope::warn!("rhai: {}", msg);
    }

    pub fn error(msg: &str) {
        slog_scope::error!("rhai: {}", msg);
    }
}

pub fn register(engine: &mut Engine) {
    let exports_module = exported_module!(exports);
    engine.register_static_module("log", exports_module.into());
}
