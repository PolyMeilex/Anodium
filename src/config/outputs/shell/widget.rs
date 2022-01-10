pub use calloop::channel::Sender;
use imgui::Ui;
use rhai::plugin::*;

pub use crate::config::eventloop::ConfigEvent;

pub trait Widget {
    fn render(&self, ui: &Ui, config_tx: &Sender<ConfigEvent>);
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
    use crate::config::outputs::shell::{
        button::Button, fps::Fps, logger::Logger, output::OutputGeometry, text::Text,
        workspace::CurrentWorkspace,
    };
    use crate::output_map::Output;

    pub fn text(text: String) -> Text {
        Text::new(text)
    }

    pub fn button(text: String) -> Button {
        Button::new(text)
    }

    pub fn fps(output: Output) -> Fps {
        Fps::new(output)
    }

    pub fn logger() -> Logger {
        Logger::new()
    }

    pub fn current_workspace(output: Output) -> CurrentWorkspace {
        CurrentWorkspace::new(output)
    }

    pub fn output_geometry(output: Output) -> OutputGeometry {
        OutputGeometry::new(output)
    }
}

pub fn register(engine: &mut Engine) {
    let widget_module = exported_module!(widget);
    engine.register_static_module("widget", widget_module.into());
}
