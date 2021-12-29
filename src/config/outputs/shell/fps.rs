use std::rc::Rc;

use imgui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use crate::output_map::Output;

use super::widget::Widget;

#[derive(Debug, Clone)]
pub struct Fps(Output);

impl Fps {
    pub fn new(output: Output) -> Self {
        Self(output)
    }
}

impl Widget for Fps {
    fn render(&self, ui: &Ui) {
        ui.text(format!("FPS: {}", self.0.get_fps() as u32));
    }
}

#[export_module]
pub mod fps {
    #[rhai_fn(global)]
    pub fn convert(fps: &mut Fps) -> Rc<dyn Widget> {
        Rc::new(fps.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let fps_module = exported_module!(fps);
    engine
        .register_static_module("fps", fps_module.into())
        .register_type::<Fps>();
}
