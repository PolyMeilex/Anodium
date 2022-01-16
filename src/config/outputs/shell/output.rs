use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use crate::output_map::Output;

use super::widget::*;

#[derive(Debug, Clone)]
pub struct OutputGeometry(Output);

impl OutputGeometry {
    pub fn new(output: Output) -> Self {
        Self(output)
    }
}

impl Widget for OutputGeometry {
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        ui.label(format!("Geometry: {:?}", self.0.geometry()));
    }
}

#[export_module]
pub mod output {
    #[rhai_fn(global)]
    pub fn convert(output_geometry: &mut OutputGeometry) -> Rc<dyn Widget> {
        Rc::new(output_geometry.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let output_module = exported_module!(output);
    engine
        .register_global_module(output_module.into())
        .register_type::<OutputGeometry>();
}
