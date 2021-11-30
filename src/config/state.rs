use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult};

#[derive(Debug, Clone)]
pub struct StateConfig();

impl StateConfig {
    pub fn new() -> Self {
        Self()
    }
}

impl From<StateConfig> for Dynamic {
    fn from(state: StateConfig) -> Self {
        rhai::Dynamic::from(state)
    }
}

#[export_module]
pub mod state_module {
    use super::StateConfig;

    pub fn test(output: &mut StateConfig) {
        slog_scope::error!("StateConfig test");
    }
}

pub fn register(engine: &mut Engine) {
    let module = exported_module!(state_module);

    engine
        .register_type::<StateConfig>()
        .register_global_module(module.into());
}
