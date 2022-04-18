use std::cell::RefCell;
use std::rc::Rc;

use rhai::plugin::*;
use smithay::utils::{Logical, Point};

use crate::state::InputState as AnodiumInputState;

#[derive(Clone)]
pub struct InputState(Rc<RefCell<AnodiumInputState>>);

impl std::fmt::Debug for InputState {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl InputState {
    pub fn new(input_state: Rc<RefCell<AnodiumInputState>>) -> Self {
        Self(input_state)
    }

    pub fn pointer_position(&self) -> Point<f64, Logical> {
        if let Ok(input_state) = self.0.try_borrow() {
            input_state.pointer_location
        } else {
            (0.0, 0.0).into()
        }
    }
}

#[export_module]
pub mod input {}

pub fn register(engine: &mut Engine) {
    let input_module = exported_module!(input);

    engine
        .register_static_module("input_module", input_module.into())
        .register_type::<InputState>();
}
