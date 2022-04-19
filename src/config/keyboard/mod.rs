use rhai::plugin::*;
use rhai::{FnPtr, Scope};

use smithay::backend::input::KeyState;
use smithay::wayland::seat::KeysymHandle;
use xkbcommon::xkb;

pub mod keybind;
pub mod modifiers_state;

pub use keybind::{KeyBindings, Keybind};
pub use modifiers_state::ConfigModifiersState;

#[derive(Debug, Default, Clone)]
pub struct Keyboard {
    key_bindings: KeyBindings,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            key_bindings: KeyBindings::new(),
        }
    }

    pub fn key_action(
        &self,
        engine: &mut rhai::Engine,
        ast: &mut rhai::AST,
        modifiers: ConfigModifiersState,
        keysym: &KeysymHandle,
        state: KeyState,
    ) {
        self.key_bindings
            .key_action(engine, ast, modifiers, keysym, state);
    }
}

#[export_module]
pub mod keyboard {
    pub fn number_keys() -> rhai::Array {
        (1..=9).map(|n| n.to_string()).map(|s| s.into()).collect()
    }

    pub fn function_keys() -> rhai::Array {
        (1..=12)
            .map(|n| format!("F{}", n))
            .map(|s| s.into())
            .collect()
    }

    #[rhai_fn(global)]
    pub fn keybind(
        keyboard: &mut Keyboard,
        modifiers_arr: rhai::Array,
        key: rhai::ImmutableString,
        fnptr: FnPtr,
    ) {
        let modifiers: ConfigModifiersState = modifiers_arr
            .iter()
            .filter_map(|k| k.clone().into_string().ok())
            .collect();

        let keysym = xkb::keysym_from_name(&key, xkb::KEYSYM_CASE_INSENSITIVE);
        let keybind = Keybind { modifiers, keysym };

        keyboard.key_bindings.insert(keybind, fnptr);
    }
}

pub fn register(engine: &mut Engine, scope: &mut Scope) -> Keyboard {
    let keyboard_module = exported_module!(keyboard);

    engine
        .register_static_module("keyboard", keyboard_module.into())
        .register_type::<Keyboard>();

    let keyboard = Keyboard::new();
    scope.set_value("Keyboard", keyboard.clone());

    keyboard
}
