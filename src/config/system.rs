use rhai::{plugin::*, Scope};

use std::process::Command;

#[derive(Debug, Clone)]
pub struct System {}

#[export_module]
pub mod system {
    #[rhai_fn(global)]
    pub fn exec(_system: &mut System, command: &str) {
        let command_split = shell_words::split(command).unwrap();

        if let Err(e) = Command::new(&command_split[0])
            .args(&command_split[1..])
            .spawn()
        {
            slog_scope::error!("failed to start command: {}, err: {:?}", command, e);
        }
    }
}

pub fn register(engine: &mut Engine, scope: &mut Scope) {
    let system_module = exported_module!(system);

    engine
        .register_global_module(system_module.into())
        .register_type::<System>();

    scope.set_value("System", System {});
}
