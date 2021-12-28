pub mod drain;

mod filter;
mod serializer;

pub use drain::ShellDrain;
use std::rc::Rc;

use imgui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use super::widget::Widget;
use drain::BUFFER;

#[derive(Debug, Clone)]
pub struct Logger();

impl Logger {
    pub fn new() -> Self {
        Self()
    }
}

impl Widget for Logger {
    fn render(&self, ui: &Ui) {
        let buffer = BUFFER.lock().unwrap();
        for line in buffer.iter() {
            ui.text(line);
        }
    }
}

#[export_module]
pub mod logger {
    pub fn widget() -> Logger {
        Logger::new()
    }

    #[rhai_fn(global)]
    pub fn convert(text: &mut Logger) -> Rc<dyn Widget> {
        Rc::new(text.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let logger_module = exported_module!(logger);
    engine
        .register_static_module("logger", logger_module.into())
        .register_type::<Logger>();
}
