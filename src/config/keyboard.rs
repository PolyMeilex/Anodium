use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Mutex;

use rhai::plugin::*;

use rhai::FnPtr;

use lazy_static::lazy_static;
use smithay::backend::input::KeyState;

use super::ConfigVM;

lazy_static! {
    static ref CALLBACKS: Callbacks = Callbacks::new();
}

struct Callbacks {
    callbacks: Mutex<HashMap<String, Vec<Callback>>>,
    keys_pressed: Mutex<HashSet<String>>,
}

impl Callbacks {
    pub fn new() -> Self {
        Self {
            callbacks: Mutex::new(HashMap::new()),
            keys_pressed: Mutex::new(HashSet::new()),
        }
    }

    pub fn clear(&self) {
        self.callbacks.lock().unwrap().clear();
    }

    pub fn insert(&self, key: &str, callback: Callback) {
        let mut callbacks = self.callbacks.lock().unwrap();

        if let Some(callbacks) = callbacks.get_mut(key) {
            callbacks.push(callback);
        } else {
            let callbacks_vec = vec![callback];
            callbacks.insert(key.to_owned(), callbacks_vec);
        }
    }

    pub fn key_action(&self, config: &ConfigVM, key: &str, state: KeyState) -> bool {
        let mut executed = false;
        let mut keys_pressed = self.keys_pressed.lock().unwrap();

        match state {
            KeyState::Pressed => keys_pressed.insert(key.to_owned()),
            KeyState::Released => keys_pressed.remove(key),
        };

        if keys_pressed.len() > 0 {
            let mut callbacks = self.callbacks.lock().unwrap();
            for (key, callbacks) in callbacks.iter() {
                if keys_pressed.contains(key) {
                    for callback in callbacks {
                        if callback.execute(config, &keys_pressed) {
                            executed = true;
                            break;
                        }
                    }
                }
            }
        }

        executed
    }
}

#[derive(Debug)]
struct Callback {
    keys: Vec<String>,
    callback: String,
}

impl Callback {
    pub fn new(callback: String, keys: Vec<String>) -> Self {
        Self { callback, keys }
    }

    pub fn execute(&self, config: &ConfigVM, keys_pressed: &HashSet<String>) -> bool {
        if self.keys.iter().all(|item| keys_pressed.contains(item)) {
            config.execute_fn(&self.callback);
            true
        } else {
            false
        }
    }
}

#[export_module]
pub mod exports {
    pub fn register(callback: FnPtr, key: &str, keys: rhai::Array) {
        let callback = callback.fn_name().to_string();

        let keys_parsed: Vec<String> = keys.iter().map(|k| format!("{}", k)).collect();
        let callback = Callback::new(callback, keys_parsed);

        CALLBACKS.insert(key, callback);
    }
}

pub fn register(engine: &mut Engine) {
    let exports_module = exported_module!(exports);
    engine.register_static_module("keyboard", exports_module.into());
}

pub fn key_action(config: &ConfigVM, key: &str, state: KeyState) -> bool {
    CALLBACKS.key_action(config, key, state)
}
