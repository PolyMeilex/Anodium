use std::cell::RefCell;
use std::rc::Rc;

use egui::CtxRef;
use rhai::plugin::*;
use rhai::Engine;

pub mod r#box;
mod button;
mod fps;
pub mod logger;
mod output;
mod text;
mod widget;
mod workspace;

use widget::*;

#[derive(Clone, Default)]
pub struct Shell {
    boxes: Rc<RefCell<Vec<r#box::Box>>>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            boxes: Default::default(),
        }
    }

    pub fn add_box(&self, r#box: r#box::Box) {
        self.boxes.borrow_mut().push(r#box);
    }

    pub fn render(&self, ctx: &CtxRef, config_tx: &Sender<ConfigEvent>) {
        for r#box in self.boxes.borrow().iter() {
            r#box.render(ctx, config_tx);
        }
    }
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "derp")
    }
}

#[export_module]
pub mod shell {
    #[rhai_fn(global)]
    pub fn add_box(shell: &mut Shell, r#box: r#box::Box) {
        shell.add_box(r#box);
    }
}

pub fn register(engine: &mut Engine) {
    let shell_module = exported_module!(shell);
    engine
        .register_static_module("shell", shell_module.into())
        .register_type::<Shell>();

    widget::register(engine);
    r#box::register(engine);
    text::register(engine);
    logger::register(engine);
    fps::register(engine);
    workspace::register(engine);
    output::register(engine);
    button::register(engine);
}
