use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use super::widget::*;
use crate::config::input::InputState;

#[derive(Debug, Clone)]
pub struct PointerPosition(InputState);

impl PointerPosition {
    pub fn new(input_state: InputState) -> Self {
        Self(input_state)
    }
}

impl Widget for PointerPosition {
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        let position = self.0.pointer_position();
        ui.label(format!("Pointer x: {:.1} y: {:.1}", position.x, position.y));
    }
}

#[export_module]
pub mod input {
    #[rhai_fn(global)]
    pub fn convert(pointer_position: &mut PointerPosition) -> Rc<dyn Widget> {
        Rc::new(pointer_position.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let input_module = exported_module!(input);
    engine
        .register_global_module(input_module.into())
        .register_type::<PointerPosition>();
}
