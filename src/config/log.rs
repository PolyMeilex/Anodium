use rhai::{plugin::*, Scope}; // a "prelude" import for macros

#[derive(Debug, Clone)]
pub struct Log {
    logger: slog::Logger,
}

impl From<Log> for Dynamic {
    fn from(log: Log) -> Self {
        rhai::Dynamic::from(log)
    }
}

#[export_module]
pub mod log {
    use super::Log;

    pub fn info(log: &mut Log, msg: &str) {
        slog::info!(log.logger, "{}", msg);
    }

    pub fn warn(log: &mut Log, msg: &str) {
        slog::warn!(log.logger, "{}", msg);
    }

    pub fn error(log: &mut Log, msg: &str) {
        slog::error!(log.logger, "{}", msg);
    }
}

pub fn register(engine: &mut Engine, scope: &mut Scope, logger: slog::Logger) {
    let log_module = exported_module!(log);

    engine
        .register_type::<Log>()
        .register_global_module(log_module.into());

    let logger = logger.new(slog::o!("rhai" => "config"));

    scope.set_value("Log", Log { logger });
}
