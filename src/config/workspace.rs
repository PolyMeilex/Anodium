use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult};

use smithay::reexports::calloop::channel::Sender;

use super::eventloop::ConfigEvent;

#[derive(Debug, Clone)]
pub struct Workspace {
    event_sender: Sender<ConfigEvent>,
}

impl Workspace {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self { event_sender }
    }
}

#[export_module]
pub mod workspace {
    #[rhai_fn(global)]
    pub fn select(workspace: &mut Workspace, name: String) {
        workspace
            .event_sender
            .send(ConfigEvent::SwitchWorkspace(name))
            .unwrap();
    }
}

pub fn register(engine: &mut Engine) {
    let workspace_module = exported_module!(workspace);

    engine
        .register_static_module("workspace", workspace_module.into())
        .register_type::<Workspace>();
}
