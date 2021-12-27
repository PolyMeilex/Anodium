use std::cell::RefCell;
use std::rc::Rc;

use imgui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use super::widget::Widget;

#[derive(Debug, Clone)]
pub struct Text(Rc<RefCell<String>>);

impl Text {
    pub fn new(text: String) -> Self {
        Self(Rc::new(RefCell::new(text)))
    }
}

impl Widget for Text {
    fn render(&self, ui: &Ui) {
        ui.text(&*self.0.borrow());
    }
}

#[export_module]
pub mod text {
    pub fn widget(text: String) -> Text {
        Text::new(text)
    }

    #[rhai_fn(global)]
    pub fn update(text: &mut Text, new_text: String) {
        *text.0.borrow_mut() = new_text;
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
