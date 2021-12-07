use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use rhai::plugin::*;
use rhai::Scope;

use rhai::FnPtr;
use smithay::backend::input::KeyState;
use xkbcommon::xkb;

use super::ConfigVM;
use super::FnCallback;

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
    fncallback: FnCallback,
    capture: Option<Capture>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Capture {
    Numbers,
    Letters,
}

impl Capture {
    pub fn is_captured(&self, key: u32) -> bool {
        match self {
            Self::Letters => (xkb::KEY_a..=xkb::KEY_z).contains(&key),
            Self::Numbers => (xkb::KEY_0..=xkb::KEY_9).contains(&key),
        }
    }
}

#[export_module]
mod capture {
    #[allow(non_snake_case)]
    pub fn Numbers() -> Capture {
        Capture::Numbers
    }
    #[allow(non_snake_case)]
    pub fn Letters() -> Capture {
        Capture::Letters
    }

    #[rhai_fn(global, get = "enum_type", pure)]
    pub fn get_type(my_enum: &mut Capture) -> String {
        match my_enum {
            Capture::Numbers => "Numbers".to_string(),
            Capture::Letters => "Letters".to_string(),
        }
    }

    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(my_enum: &mut Capture) -> String {
        format!("{:?}", my_enum)
    }

    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(my_enum: &mut Capture, my_enum2: Capture) -> bool {
        my_enum == &my_enum2
    }

    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq(my_enum: &mut Capture, my_enum2: Capture) -> bool {
        my_enum != &my_enum2
    }
}

impl Callback {
    pub fn new(fncallback: FnCallback, keys: Vec<u32>, capture: Option<Capture>) -> Self {
        Self {
            fncallback,
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
                config.execute_callback(
                    self.fncallback.clone(),
                    &mut [rhai::Dynamic::from(captured)],
                );
            } else {
                config.execute_callback(self.fncallback.clone(), &mut []);
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
}

#[export_module]
pub mod callbacks {
    #[rhai_fn(global)]
    pub fn register(
        context: NativeCallContext,
        callbacks: &mut Callbacks,
        fnptr: FnPtr,
        key: &str,
        keys: rhai::Array,
    ) {
        let callback = FnCallback::new(fnptr, context);

        let keys_parsed: Vec<u32> = keys
            .iter()
            .map(|k| xkb::keysym_from_name(&format!("{}", k), xkb::KEYSYM_CASE_INSENSITIVE))
            .collect();
        let callback = Callback::new(callback, keys_parsed, None);
        callbacks.insert(key, callback);
    }

    #[rhai_fn(global)]
    pub fn register_capture(
        context: NativeCallContext,
        callbacks: &mut Callbacks,
        fnptr: FnPtr,
        key: &str,
        keys: rhai::Array,
        capture: Capture,
    ) {
        let callback = FnCallback::new(fnptr, context);

        let keys_parsed: Vec<u32> = keys
            .iter()
            .map(|k| xkb::keysym_from_name(&format!("{}", k), xkb::KEYSYM_CASE_INSENSITIVE))
            .collect();
        let callback = Callback::new(callback, keys_parsed, Some(capture));
        callbacks.insert(key, callback);
    }
}

pub fn register(scope: &mut Scope, engine: &mut Engine) {
    let capture_module = exported_module!(capture);

    let keyboard_module = exported_module!(keyboard);
    let callbacks_module = exported_module!(callbacks);
    engine
        .register_static_module("KeyCapture", capture_module.into())
        .register_static_module("keyboard", keyboard_module.into())
        .register_static_module("callbacks", callbacks_module.into())
        .register_type::<Capture>()
        .register_type::<Keyboard>()
        .register_type::<Callbacks>();
}
