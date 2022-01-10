use std::cell::RefCell;
use std::rc::Rc;

use imgui::Ui;
use rhai::plugin::*;
use rhai::Engine;
use rhai::FnPtr;

use super::widget::*;

#[derive(Debug)]
struct ButtonInner {
    label: String,
    click: Option<FnPtr>,
}

#[derive(Debug, Clone)]
pub struct Button(Rc<RefCell<ButtonInner>>);

impl Button {
    pub fn new(label: String) -> Self {
        Self(Rc::new(RefCell::new(ButtonInner { label, click: None })))
    }
}

impl Widget for Button {
    fn render(&self, ui: &Ui, config_tx: &Sender<ConfigEvent>) {
        let button = self.0.borrow();
        if ui.button(&button.label) {
            if let Some(click) = &button.click {
                config_tx.send(ConfigEvent::Shell(click.clone())).unwrap();
            }
        }
    }
}

#[export_module]
pub mod button {
    #[rhai_fn(global)]
    pub fn label(button: &mut Button, label: String) {
        button.0.borrow_mut().label = label;
    }

    #[rhai_fn(global)]
    pub fn click(button: &mut Button, click: FnPtr) {
        button.0.borrow_mut().click = Some(click);
    }

    #[rhai_fn(global)]
    pub fn convert(button: &mut Button) -> Rc<dyn Widget> {
        Rc::new(button.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let button_module = exported_module!(button);
    engine
        .register_static_module("button", button_module.into())
        .register_type::<Button>();
}
