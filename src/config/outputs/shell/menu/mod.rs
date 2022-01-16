use std::cell::RefCell;
use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::{Engine, FnPtr};

use super::widget::*;

use self::item::MenuItem;
use self::label::Label;

mod item;
mod label;

struct MenuInner {
    label: String,
    items: Vec<Rc<dyn MenuItem>>,
}

#[derive(Clone)]
pub struct Menu(Rc<RefCell<MenuInner>>);

impl Menu {
    pub fn new(label: String) -> Self {
        Self(Rc::new(RefCell::new(MenuInner {
            label,
            items: vec![],
        })))
    }
}

impl Widget for Menu {
    fn render(&self, ui: &mut Ui, config_tx: &Sender<ConfigEvent>) {
        let inner = self.0.borrow();
        egui::menu::menu_button(ui, &inner.label, |ui| {
            for item in &inner.items {
                item.render(ui, config_tx);
            }
        });
    }
}

#[export_module]
pub mod menu {
    #[rhai_fn(global)]
    pub fn convert(menu: &mut Menu) -> Rc<dyn Widget> {
        Rc::new(menu.clone())
    }

    #[rhai_fn(global)]
    pub fn add_item(menu: &mut Menu, item: Rc<dyn MenuItem>) {
        menu.0.borrow_mut().items.push(item);
    }

    pub fn label(text: String, fnptr: FnPtr) -> Label {
        Label::new(text, fnptr)
    }
}

pub fn register(engine: &mut Engine) {
    let menu_module = exported_module!(menu);
    engine
        .register_static_module("menu", menu_module.into())
        .register_type::<Menu>();

    label::register(engine);
}
