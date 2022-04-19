use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rhai::plugin::*;
use rhai::FnPtr;

use smithay::backend::input::KeyState;
use smithay::wayland::seat::KeysymHandle;

use super::ConfigModifiersState;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keybind {
    pub modifiers: ConfigModifiersState,
    pub keysym: u32,
}

#[derive(Debug, Default, Clone)]
pub struct KeyBindings {
    callbacks: Rc<RefCell<HashMap<Keybind, Vec<FnPtr>>>>,
}

impl KeyBindings {
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
