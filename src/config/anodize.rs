use rhai::{plugin::*, Scope};
use rhai::{Dynamic, EvalAltResult, FnPtr};

use smithay::reexports::calloop::channel::Sender;

use super::keyboard::Keyboard;
use super::system::System;

#[derive(Debug, Clone)]
pub struct Anodize {
    pub keyboard: Keyboard,
    system: System,
}

impl Anodize {
    pub fn new() -> Self {
        Self {
            keyboard: Keyboard::new(),
            system: System::new(),
        }
    }

    pub fn key_action() {}
}

impl From<Anodize> for Dynamic {
    fn from(anodize: Anodize) -> Self {
        rhai::Dynamic::from(anodize)
    }
}

#[export_module]
pub mod anodize_module {
    use super::Anodize;

    #[rhai_fn(get = "keyboard", pure)]
    pub fn get_keyboard(anodize: &mut Anodize) -> Keyboard {
        anodize.keyboard.clone()
    }

    #[rhai_fn(get = "system", pure)]
    pub fn get_system(anodize: &mut Anodize) -> System {
        anodize.system.clone()
    }
}

pub fn register(scope: &mut Scope, engine: &mut Engine) -> Anodize {
    let anodize = Anodize::new();
    let module = exported_module!(anodize_module);

    engine
        .register_type::<Anodize>()
        .register_global_module(module.into());

    scope.push_constant("anodize", anodize.clone());
    anodize
}
