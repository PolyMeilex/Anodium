use imgui::Ui;
use rhai::plugin::*;

pub trait Widget {
    fn render(&self, ui: &Ui);
}

impl std::fmt::Debug for Box<dyn Widget> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", "derp")
    }
}

impl Clone for Box<dyn Widget> {
    fn clone(&self) -> Box<dyn Widget> {
        self.to_owned()
    }
}

#[export_module]
pub mod widget {
    use crate::config::outputs::shell::{fps::Fps, logger::Logger, text::Text};
    use crate::output_map::Output;

    pub fn text(text: String) -> Text {
        Text::new(text)
    }

    pub fn fps(output: Output) -> Fps {
        Fps::new(output)
    }

    pub fn logger() -> Logger {
        Logger::new()
    }
}

pub fn register(engine: &mut Engine) {
    let widget_module = exported_module!(widget);
    engine.register_static_module("widget", widget_module.into());
}
