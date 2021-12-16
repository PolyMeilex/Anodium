use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use rhai::plugin::*;

use rhai::FnPtr;
use smithay::backend::input::KeyState;
use xkbcommon::xkb;

use super::ConfigVM;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Callbacks {
    callbacks: Rc<RefCell<HashMap<u32, Vec<Callback>>>>,
}

impl Callbacks {
    pub fn new() -> Self {
        Self {
            callbacks: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.callbacks.borrow_mut().clear();
    }

    pub fn insert(&self, key: &str, callback: Callback) {
        let key = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);

        let mut callbacks = self.callbacks.borrow_mut();

        if let Some(callbacks) = callbacks.get_mut(&key) {
            callbacks.push(callback);
        } else {
            let callbacks_vec = vec![callback];
            callbacks.insert(key.to_owned(), callbacks_vec);
        }
    }

    pub fn key_action(
        &self,
        config: &ConfigVM,
        current_key: u32,
        _state: KeyState,
        keys_pressed: &HashSet<u32>,
    ) -> bool {
        let mut executed = false;

        if keys_pressed.len() > 0 {
            let callbacks = self.callbacks.borrow();
            for (key, callbacks) in callbacks.iter() {
                if keys_pressed.contains(key) {
                    for callback in callbacks {
                        if let Some(capture) = &callback.capture {
                            if capture.is_captured(current_key)
                                && current_key != *key
                                && !callback.keys.contains(&current_key)
                            {
                                if callback.execute(config, keys_pressed, Some(current_key)) {
                                    executed = true;
                                    break;
                                }
                            }
                        } else {
                            if callback.execute(config, keys_pressed, None) {
                                executed = true;
                                break;
                            }
                        }
                    }
                }
            }
        }

        executed
    }
}

#[derive(Debug, Clone)]
pub struct Callback {
    keys: Vec<u32>,
    fnptr: FnPtr,
    capture: Option<Capture>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Capture {
    Numbers,
    Letters,
    Functions,
}

impl Capture {
    pub fn is_captured(&self, key: u32) -> bool {
        match self {
            Self::Letters => (xkb::KEY_a..=xkb::KEY_z).contains(&key),
            Self::Numbers => (xkb::KEY_0..=xkb::KEY_9).contains(&key),
            Self::Functions => (xkb::KEY_F1..=xkb::KEY_F12).contains(&key),
        }
    }
}

impl Callback {
    pub fn new(fnptr: FnPtr, keys: Vec<u32>, capture: Option<Capture>) -> Self {
        Self {
            fnptr,
            keys,
            capture,
        }
    }

    pub fn execute(
        &self,
        config: &ConfigVM,
        keys_pressed: &HashSet<u32>,
        captured: Option<u32>,
    ) -> bool {
        if self.keys.iter().all(|item| keys_pressed.contains(item)) {
            if let Some(captured) = captured {
                let captured = ::xkbcommon::xkb::keysym_get_name(captured);
                config.execute_fnptr(self.fnptr.clone(), (captured,));
            } else {
                config.execute_fnptr(self.fnptr.clone(), ());
            }
            true
        } else {
            false
        }
    }
}

#[export_module]
pub mod keyboard {
    #[rhai_fn(get = "callbacks", pure, global)]
    pub fn get_callbacks(keyboard: &mut Keyboard) -> Callbacks {
        keyboard.callbacks.clone()
    }

    pub fn numbers() -> Capture {
        Capture::Numbers
    }

    pub fn letters() -> Capture {
        Capture::Letters
    }

    pub fn functions() -> Capture {
        Capture::Functions
    }
}

#[export_module]
pub mod callbacks {
    #[rhai_fn(global)]
    pub fn register(callbacks: &mut Callbacks, fnptr: FnPtr, key: &str, keys: rhai::Array) {
        let keys_parsed: Vec<u32> = keys
            .iter()
            .map(|k| xkb::keysym_from_name(&format!("{}", k), xkb::KEYSYM_CASE_INSENSITIVE))
            .collect();
        let callback = Callback::new(fnptr, keys_parsed, None);
        callbacks.insert(key, callback);
    }

    #[rhai_fn(global)]
    pub fn register_capture(
        callbacks: &mut Callbacks,
        fnptr: FnPtr,
        key: &str,
        keys: rhai::Array,
        capture: Capture,
    ) {
        let keys_parsed: Vec<u32> = keys
            .iter()
            .map(|k| xkb::keysym_from_name(&format!("{}", k), xkb::KEYSYM_CASE_INSENSITIVE))
            .collect();
        let callback = Callback::new(fnptr, keys_parsed, Some(capture));
        callbacks.insert(key, callback);
    }
}

pub fn register(engine: &mut Engine) {
    let keyboard_module = exported_module!(keyboard);
    let callbacks_module = exported_module!(callbacks);
    engine
        .register_static_module("keyboard", keyboard_module.into())
        .register_static_module("callbacks", callbacks_module.into())
        .register_type::<Capture>()
        .register_type::<Keyboard>()
        .register_type::<Callbacks>();
}
