use rhai::plugin::*;

use crate::output_map::{Output, OutputMap}; // a "prelude" import for macros

#[derive(Debug, Clone)]
pub struct Outputs(OutputMap);

impl Outputs {
    pub fn new(output_map: OutputMap) -> Self {
        Self(output_map)
    }
}

#[export_module]
pub mod outputs {
    #[rhai_fn(global)]
    pub fn info(output: &mut Output) {
        slog_scope::info!("output: {:?}", output);
    }

    #[rhai_fn(global)]
    pub fn set_wallpaper(output: &mut Output, path: &str) {
        output.set_wallpaper(path);
    }
}

impl IntoIterator for Outputs {
    type Item = Output;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub fn register(engine: &mut Engine) {
    let outputs_module = exported_module!(outputs);
    engine
        .register_static_module("outputs", outputs_module.into())
        .register_type::<Outputs>()
        .register_type::<Output>()
        .register_iterator::<Outputs>();
}
