use std::rc::Rc;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use crate::output_manager::Output;

use super::widget::*;

#[derive(Debug, Clone)]
pub struct CurrentWorkspace(Output);

impl CurrentWorkspace {
    pub fn new(output: Output) -> Self {
        Self(output)
    }
}

impl Widget for CurrentWorkspace {
    fn render(&self, ui: &Ui, _config_tx: &Sender<ConfigEvent>) {
        todo!();
        // ui.text(format!("Workspace: {}", self.0.active_workspace()));
    }
}

#[export_module]
pub mod workspace {
    #[rhai_fn(global)]
    pub fn convert(current_workspace: &mut CurrentWorkspace) -> Rc<dyn Widget> {
        Rc::new(current_workspace.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let workspace_module = exported_module!(workspace);
    engine
        .register_global_module(workspace_module.into())
        .register_type::<CurrentWorkspace>();
}
