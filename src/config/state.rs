use std::cell::RefCell;
use std::rc::Rc;

use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult};

use crate::desktop_layout::DesktopLayout;

#[derive(Debug, Clone)]
pub struct StateConfig(Rc<RefCell<DesktopLayout>>);

impl StateConfig {
    pub fn new(state: Rc<RefCell<DesktopLayout>>) -> Self {
        Self(state)
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
