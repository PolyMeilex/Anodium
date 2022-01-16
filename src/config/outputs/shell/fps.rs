use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use crate::output_manager::Output;

use super::widget::*;

#[derive(Debug, Clone)]
pub struct Fps(Output);

impl Fps {
    pub fn new(output: Output) -> Self {
        Self(output)
    }
}

impl Widget for Fps {
    fn render(&self, ui: &Ui, _config_tx: &Sender<ConfigEvent>) {
        todo!();
        // ui.text(format!("FPS: {}", self.0.get_fps() as u32));
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
