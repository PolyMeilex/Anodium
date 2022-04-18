use std::{cell::RefCell, collections::HashMap, hash::Hash, rc::Rc};

use rhai::plugin::*;
use rhai::{FnPtr, Scope};

use smithay::backend::input::KeyState;
use smithay::wayland::seat::{KeysymHandle, ModifiersState};
use xkbcommon::xkb;

#[derive(Debug, Default, Clone)]
pub struct Keyboard {
    pub callbacks: Callbacks,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            callbacks: Callbacks::new(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConfigModifiersState {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
}

impl From<ModifiersState> for ConfigModifiersState {
    fn from(m: ModifiersState) -> Self {
        Self {
            ctrl: m.ctrl,
            alt: m.alt,
            shift: m.shift,
            logo: m.logo,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keybind {
    modifiers: ConfigModifiersState,
    keysym: u32,
}

#[derive(Debug, Default, Clone)]
pub struct Callbacks {
    callbacks: Rc<RefCell<HashMap<Keybind, Vec<FnPtr>>>>,
}

impl Callbacks {
    pub fn new() -> Self {
        Self {
            callbacks: Default::default(),
        }
    }

    pub fn insert(&self, keybind: Keybind, callback: FnPtr) {
        let mut callbacks = self.callbacks.borrow_mut();

        if let Some(callbacks) = callbacks.get_mut(&keybind) {
            callbacks.push(callback);
        } else {
            callbacks.insert(keybind, vec![callback]);
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
        let keysyms = keysym.raw_syms();
        let keysym = keysyms[0];

        let keybind = Keybind { modifiers, keysym };

        if state == KeyState::Pressed {
            if let Some(cbs) = self.callbacks.borrow_mut().get_mut(&keybind) {
                for fnptr in cbs {
                    let _: Option<Dynamic> = fnptr.call(engine, ast, ()).ok();
                }
            }
        }
    }
}

#[export_module]
pub mod keyboard {
    #[rhai_fn(get = "callbacks", pure, global)]
    pub fn get_callbacks(keyboard: &mut Keyboard) -> Callbacks {
        keyboard.callbacks.clone()
    }

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
    pub fn register(
        keyboard: &mut Keyboard,
        modifiers_arr: rhai::Array,
        key: rhai::ImmutableString,
        fnptr: FnPtr,
    ) {
        let mut modifiers = ConfigModifiersState::default();
        for m in modifiers_arr
            .iter()
            .filter_map(|k| k.clone().into_string().ok())
        {
            match m.as_str() {
                "ctrl" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "logo" => modifiers.logo = true,
                _ => {}
            }
        }

        let keysym = xkb::keysym_from_name(&key, xkb::KEYSYM_CASE_INSENSITIVE);
        let keybind = Keybind { modifiers, keysym };

        keyboard.callbacks.insert(keybind, fnptr);
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
