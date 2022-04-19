use rhai::{plugin::*, Scope};

use std::{cell::RefCell, rc::Rc};

pub mod layout;
pub use layout::{OutputDescriptor, OutputLayout};

#[derive(Debug, Default, Clone)]
pub struct Outputs {
    layout: Rc<RefCell<OutputLayout>>,
}

impl Outputs {
    pub fn layout(&self) -> OutputLayout {
        self.layout.borrow().clone()
    }
}

#[export_module]
pub mod outputs {
    #[rhai_fn(set = "layout")]
    pub fn set_layout(outputs: &mut Outputs, layout: rhai::Array) {
        let layout = layout
            .iter()
            .map(|output| rhai::serde::from_dynamic(output).unwrap())
            .collect();

        outputs.layout.borrow_mut().0 = layout;
    }
}

pub fn register(engine: &mut Engine, scope: &mut Scope) -> Outputs {
    let system_module = exported_module!(outputs);

    engine
        .register_global_module(system_module.into())
        .register_type::<Outputs>();

    let outputs = Outputs::default();
    scope.set_value("Outputs", outputs.clone());

    outputs
}
