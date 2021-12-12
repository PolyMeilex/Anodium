use rhai::Dynamic;
use rhai::{plugin::*, Scope};

use smithay::reexports::calloop::channel::Sender;

use crate::output_map::OutputMap;

use super::eventloop::ConfigEvent;
use super::keyboard::Keyboard;
use super::log::Log;
use super::outputs::Outputs;
use super::system::System;
use super::windows::Windows;
use super::workspace::Workspace;

#[derive(Debug, Clone)]
pub struct Anodize {
    pub keyboard: Keyboard,
    system: System,
    workspace: Workspace,
    pub windows: Windows,
    log: Log,
    outputs: Outputs,
}

impl Anodize {
    pub fn new(event_sender: Sender<ConfigEvent>, output_map: OutputMap) -> Self {
        Self {
            keyboard: Keyboard::new(),
            system: System::new(event_sender.clone()),
            workspace: Workspace::new(event_sender.clone()),
            windows: Windows::new(event_sender.clone()),
            log: Log::new(),
            outputs: Outputs::new(output_map),
        }
    }
}

impl From<Anodize> for Dynamic {
    fn from(anodize: Anodize) -> Self {
        rhai::Dynamic::from(anodize)
    }
}

#[export_module]
pub mod anodize_module {
    use super::Anodize;

    #[rhai_fn(get = "keyboard", pure)]
    pub fn get_keyboard(anodize: &mut Anodize) -> Keyboard {
        anodize.keyboard.clone()
    }

    #[rhai_fn(get = "system", pure)]
    pub fn get_system(anodize: &mut Anodize) -> System {
        anodize.system.clone()
    }

    #[rhai_fn(get = "workspace", pure)]
    pub fn get_workspace(anodize: &mut Anodize) -> Workspace {
        anodize.workspace.clone()
    }

    #[rhai_fn(get = "windows", pure)]
    pub fn get_windows(anodize: &mut Anodize) -> Windows {
        anodize.windows.clone()
    }

    #[rhai_fn(get = "log", pure)]
    pub fn get_log(anodize: &mut Anodize) -> Log {
        anodize.log.clone()
    }

    #[rhai_fn(get = "outputs", pure)]
    pub fn get_outputs(anodize: &mut Anodize) -> Outputs {
        anodize.outputs.clone()
    }
}

pub fn register(
    scope: &mut Scope,
    engine: &mut Engine,
    event_sender: Sender<ConfigEvent>,
    output_map: OutputMap,
) -> Anodize {
    let anodize = Anodize::new(event_sender, output_map);
    let module = exported_module!(anodize_module);

    engine
        .register_type::<Anodize>()
        .register_global_module(module.into());

    scope.push_constant("anodize", anodize.clone());
    anodize
}
