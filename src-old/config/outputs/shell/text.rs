use std::cell::RefCell;
use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use super::widget::*;

#[derive(Debug)]
struct TextInner {
    text: String,
    updated: bool,
}

#[derive(Debug, Clone)]
pub struct Text(Rc<RefCell<TextInner>>);

impl Text {
    pub fn new(text: String) -> Self {
        Self(Rc::new(RefCell::new(TextInner {
            text,
            updated: true,
        })))
    }
}

impl Widget for Text {
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        let mut inner = self.0.borrow_mut();
        if inner.updated {
            ui.ctx().request_repaint();
            inner.updated = false;
        }

        ui.label(&inner.text);
    }
}

#[export_module]
pub mod text {
    #[rhai_fn(global)]
    pub fn update(text: &mut Text, new_text: String) {
        let mut inner = text.0.borrow_mut();

        inner.text = new_text;
        inner.updated = true;
    }

    #[rhai_fn(global)]
    pub fn convert(text: &mut Text) -> Rc<dyn Widget> {
        Rc::new(text.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let text_module = exported_module!(text);
    engine
        .register_static_module("text", text_module.into())
        .register_type::<Text>();
}
