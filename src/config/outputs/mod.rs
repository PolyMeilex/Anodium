use std::cell::RefCell;
use std::rc::Rc;

use rhai::{plugin::*, Array, AST};
use rhai::{FnPtr, INT};

use smithay::wayland::output::Mode;

use crate::output_manager::{Output, OutputDescriptor, OutputManager};

pub mod shell;

#[derive(Debug, Clone)]
pub struct Modes(Vec<Mode>);

impl Modes {
    pub fn get(&mut self, index: i64) -> Dynamic {
        if let Some(mode) = self.0.get(index as usize).cloned() {
            Dynamic::from(mode)
        } else {
            Dynamic::from(())
        }
    }
}

impl IntoIterator for Modes {
    type Item = Mode;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone)]
pub struct Outputs {
    output_map: OutputManager,
    on_rearrange: Rc<RefCell<Option<FnPtr>>>,
    on_mode_select: Rc<RefCell<Option<FnPtr>>>,
    on_new: Rc<RefCell<Option<FnPtr>>>,
}

impl Outputs {
    pub fn new(output_map: OutputManager) -> Self {
        Self {
            output_map,
            on_rearrange: Default::default(),
            on_mode_select: Default::default(),
            on_new: Default::default(),
        }
    }

    pub fn on_rearrange(&self, engine: &Engine, ast: &AST) {
        if let Some(on_rearrange) = self.on_rearrange.borrow().clone() {
            let _: Dynamic = on_rearrange.call(engine, ast, ()).unwrap();
        } else {
            warn!("on_rearrange not configured");
        }
    }

    pub fn on_mode_select(
        &self,
        engine: &Engine,
        ast: &AST,
        desc: &OutputDescriptor,
        modes: &[Mode],
    ) -> Option<Mode> {
        if let Some(on_mode_select) = self.on_mode_select.borrow().clone() {
            let modes = Modes(modes.to_vec());

            let res: Mode = on_mode_select
                .call(engine, ast, (desc.clone(), modes))
                .unwrap();

            Some(res)
        } else {
            warn!("on_mode_select not configured");
            None
        }
    }

    pub fn on_new(&self, engine: &Engine, ast: &AST, output: Output) {
        if let Some(on_new) = self.on_new.borrow().clone() {
            let _result: () = on_new.call(engine, ast, (output,)).unwrap();
        } else {
            error!("on_new not configured");
        }
    }

    pub fn get(&mut self, index: INT) -> Dynamic {
        if let Some(output) = self.output_map.outputs().get(index as usize).cloned() {
            Dynamic::from(output)
        } else {
            Dynamic::from(())
        }
    }
}
#[export_module]
pub mod output_descriptor {
    #[rhai_fn(get = "name", pure)]
    pub fn name(desc: &mut OutputDescriptor) -> ImmutableString {
        desc.name.clone().into()
    }

    #[rhai_fn(get = "manufacturer", pure)]
    pub fn manufacturer(desc: &mut OutputDescriptor) -> ImmutableString {
        desc.physical_properties.make.clone().into()
    }

    #[rhai_fn(get = "model", pure)]
    pub fn model(desc: &mut OutputDescriptor) -> ImmutableString {
        desc.physical_properties.model.clone().into()
    }
}

#[export_module]
pub mod modes {
    #[rhai_fn(get = "w", pure)]
    pub fn w(mode: &mut Mode) -> INT {
        mode.size.w as _
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(mode: &mut Mode) -> INT {
        mode.size.h as _
    }

    #[rhai_fn(get = "refresh", pure)]
    pub fn refresh(mode: &mut Mode) -> INT {
        (mode.refresh / 1000) as _
    }

    #[rhai_fn(global)]
    pub fn find(modes: &mut Modes, w: INT, h: INT, refresh: INT) -> Dynamic {
        if let Some(mode) = modes.0.iter().find(|m| {
            m.size.w as i64 == w && m.size.h as i64 == h && m.refresh as i64 == refresh * 1000
        }) {
            rhai::Dynamic::from(*mode)
        } else {
            rhai::Dynamic::UNIT
        }
    }
}

#[export_module]
pub mod outputs {
    #[rhai_fn(get = "name", pure)]
    pub fn name(output: &mut Output) -> ImmutableString {
        output.name().into()
    }

    #[rhai_fn(get = "w", pure)]
    pub fn w(output: &mut Output) -> INT {
        output.current_mode().unwrap().size.w as _
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(output: &mut Output) -> INT {
        output.current_mode().unwrap().size.h as _
    }

    #[rhai_fn(get = "modes", pure)]
    pub fn modes(output: &mut Output) -> Modes {
        Modes(output.possible_modes())
    }

    #[rhai_fn(get = "shell", pure)]
    pub fn shell(output: &mut Output) -> shell::Shell {
        output.egui_shell().clone()
    }

    #[rhai_fn(global)]
    pub fn set_wallpaper(_output: &mut Output, _path: &str) {
        todo!("Let's just spawn sway-bg client, it's better and easier anyway. (maybe implement anobg/anopaper or something like that at some point)");
        // output.set_wallpaper(path);
    }

    #[rhai_fn(global)]
    pub fn update_mode(_output: &mut Output, _mode: Mode) {
        todo!("Send event using event emiter");
        // output.update_mode(mode);
    }

    #[rhai_fn(global)]
    pub fn on_rearrange(output: &mut Outputs, fnptr: FnPtr) {
        *output.on_rearrange.borrow_mut() = Some(fnptr);
    }

    #[rhai_fn(global)]
    pub fn on_mode_select(output: &mut Outputs, fnptr: FnPtr) {
        *output.on_mode_select.borrow_mut() = Some(fnptr);
    }

    #[rhai_fn(global)]
    pub fn on_new(output: &mut Outputs, fnptr: FnPtr) {
        *output.on_new.borrow_mut() = Some(fnptr);
    }
}

impl IntoIterator for Outputs {
    type Item = Output;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.output_map.into_iter()
    }
}

pub fn register(engine: &mut Engine) {
    let outputs_module = exported_module!(outputs);
    let output_desc_module = exported_module!(output_descriptor);
    let modes_module = exported_module!(modes);
    engine
        .register_static_module("outpt_descriptor", output_desc_module.into())
        .register_static_module("outputs", outputs_module.into())
        .register_static_module("modes", modes_module.into())
        .register_type::<OutputDescriptor>()
        .register_type::<Outputs>()
        .register_type::<Output>()
        .register_type::<Mode>()
        .register_type::<Modes>()
        .register_iterator::<Outputs>()
        .register_indexer_get(Outputs::get)
        .register_indexer_get(Modes::get)
        .register_iterator::<Modes>();

    shell::register(engine);
}
