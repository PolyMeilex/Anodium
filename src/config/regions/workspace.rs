use rhai::plugin::*;

use crate::region_manager::Workspace;

#[export_module]
pub mod workspace {
    pub fn create(name: String) -> Workspace {
        Workspace::new(name)
    }
}

pub fn register(engine: &mut Engine) {
    let workspace_module = exported_module!(workspace);
    engine
        .register_static_module("workspace", workspace_module.into())
        .register_type::<Workspace>();
}
