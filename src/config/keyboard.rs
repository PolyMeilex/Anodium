use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Mutex;

use rhai::plugin::*;

use rhai::FnPtr;

use lazy_static::lazy_static;
use smithay::backend::input::KeyState;
use xkbcommon::xkb;

use super::state::StateConfig;
use super::ConfigVM;

lazy_static! {
    static ref CALLBACKS: Callbacks = Callbacks::new();
}

struct Callbacks {
    callbacks: Mutex<HashMap<u32, Vec<Callback>>>,
}

impl Callbacks {
    pub fn new() -> Self {
        Self {
            callbacks: Mutex::new(HashMap::new()),
        }
    }

    pub fn clear(&self) {
        self.callbacks.lock().unwrap().clear();
    }

    pub fn insert(&self, key: &str, callback: Callback) {
        let key = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);

        let mut callbacks = self.callbacks.lock().unwrap();

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
        key: &str,
        state: KeyState,
        keys_pressed: &HashSet<u32>,
    ) -> bool {
        let mut executed = false;

        if keys_pressed.len() > 0 {
            let callbacks = self.callbacks.lock().unwrap();
            for (key, callbacks) in callbacks.iter() {
                if keys_pressed.contains(key) {
                    for callback in callbacks {
                        if callback.execute(config, keys_pressed) {
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
    keys: Vec<u32>,
    callback: String,
}

impl Callback {
    pub fn new(callback: String, keys: Vec<u32>) -> Self {
        Self { callback, keys }
    }

    pub fn execute(&self, config: &ConfigVM, keys_pressed: &HashSet<u32>) -> bool {
        if self.keys.iter().all(|item| keys_pressed.contains(item)) {
            let state = StateConfig::new();
            config.execute_fn_with_state(&self.callback, state);
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

        let keys_parsed: Vec<u32> = keys
            .iter()
            .map(|k| xkb::keysym_from_name(&format!("{}", k), xkb::KEYSYM_CASE_INSENSITIVE))
            .collect();
        let callback = Callback::new(callback, keys_parsed);

        CALLBACKS.insert(key, callback);
    }
}

pub fn register(engine: &mut Engine) {
    let exports_module = exported_module!(exports);
    engine.register_static_module("keyboard", exports_module.into());
}

pub fn key_action(
    config: &ConfigVM,
    key: &str,
    state: KeyState,
    keys_pressed: &HashSet<u32>,
) -> bool {
    CALLBACKS.key_action(config, key, state, keys_pressed)
}

pub fn callbacks_clear() {
    CALLBACKS.clear();
}
