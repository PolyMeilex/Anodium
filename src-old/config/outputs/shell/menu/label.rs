use std::cell::RefCell;
use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::{Engine, FnPtr};

use super::item::*;

#[derive(Debug)]
struct LabelInner {
    label: String,
    fnptr: FnPtr,
}

#[derive(Debug, Clone)]
pub struct Label(Rc<RefCell<LabelInner>>);

impl Label {
    pub fn new(label: String, fnptr: FnPtr) -> Self {
        Self(Rc::new(RefCell::new(LabelInner { label, fnptr })))
    }
}

impl MenuItem for Label {
    fn render(&self, ui: &mut Ui, config_tx: &Sender<ConfigEvent>) {
        let inner = self.0.borrow();
        if ui.button(&inner.label).clicked() {
            config_tx
                .send(ConfigEvent::Shell(inner.fnptr.clone()))
                .unwrap();
        }
    }
}

#[export_module]
pub mod label {
    #[rhai_fn(global)]
    pub fn update(text: &mut Label, new_text: String) {
        text.0.borrow_mut().label = new_text;
    }

    #[rhai_fn(global)]
    pub fn convert(text: &mut Label) -> Rc<dyn MenuItem> {
        Rc::new(text.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let label_module = exported_module!(label);
    engine
        .register_static_module("label", label_module.into())
        .register_type::<Label>();
}
