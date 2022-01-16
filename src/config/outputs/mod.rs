use std::cell::RefCell;
use std::rc::Rc;

use rhai::{plugin::*, Array, AST};
use rhai::{FnPtr, INT};

use smithay::wayland::output::Mode;

use crate::output_manager::{Output, OutputManager};

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
    on_new: Rc<RefCell<Option<FnPtr>>>,
}

impl Outputs {
    pub fn new(output_map: OutputManager) -> Self {
        Self {
            output_map,
            on_rearrange: Default::default(),
            on_new: Default::default(),
        }
    }

    pub fn on_rearrange(
        &self,
        engine: &Engine,
        ast: &AST,
        outputs: Vec<Output>,
    ) -> Option<Vec<(i32, i32)>> {
        if let Some(on_rearrange) = self.on_rearrange.borrow().clone() {
            let outputs: Array = outputs.into_iter().map(|o| Dynamic::from(o)).collect();

            let res: Array = on_rearrange.call(engine, ast, (outputs,)).unwrap();
            let res: Vec<(i32, i32)> = res
                .into_iter()
                .map(|item| item.try_cast().unwrap())
                .map(|pos: Array| pos.into_iter().map(|p| p.try_cast().unwrap()).collect())
                .map(|pos: Vec<INT>| (pos[0] as _, pos[1] as _))
                .collect();

            Some(res)
        } else {
            error!("on_rearrange not configured");
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

    pub fn get(&mut self, index: i64) -> Dynamic {
        todo!();
        // if let Some(output) = self.output_map.find_by_index(index as usize) {
        //     Dynamic::from(output)
        // } else {
        //     Dynamic::from(())
        // }
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
    pub fn filter(modes: &mut Modes, w: INT, h: INT, refresh: INT) -> Dynamic {
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
        // output.name().into()
        todo!();
    }

    #[rhai_fn(get = "w", pure)]
    pub fn w(output: &mut Output) -> INT {
        output.current_mode().unwrap().size.w as _
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(output: &mut Output) -> INT {
        output.current_mode().unwrap().size.h as _
    }

    #[rhai_fn(get = "x", pure)]
    pub fn get_x(output: &mut Output) -> INT {
        // output.geometry().loc.x as _
        todo!();
    }

    #[rhai_fn(get = "y", pure)]
    pub fn y(output: &mut Output) -> INT {
        // output.geometry().loc.y as _
        todo!();
    }

    #[rhai_fn(get = "modes", pure)]
    pub fn modes(output: &mut Output) -> Modes {
        // Modes(output.possible_modes())
        todo!();
    }

    #[rhai_fn(get = "shell", pure)]
    pub fn shell(output: &mut Output) -> shell::Shell {
        // output.shell()
        todo!();
    }

    #[rhai_fn(set = "x", pure)]
    pub fn x(output: &mut Output, x: INT) {
        todo!();
        // let mut location = output.location();
        // location.x = x as _;
        // output.set_location(location);
        // let geometry = output.geometry();
        // output.layer_map_mut().arange(geometry);
    }

    #[rhai_fn(global)]
    pub fn set_wallpaper(output: &mut Output, path: &str) {
        todo!();
        // output.set_wallpaper(path);
    }

    #[rhai_fn(global)]
    pub fn update_mode(output: &mut Output, mode: Mode) {
        todo!();
        // output.update_mode(mode);
    }

    #[rhai_fn(global)]
    pub fn on_rearrange(output: &mut Outputs, fnptr: FnPtr) {
        *output.on_rearrange.borrow_mut() = Some(fnptr);
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
        todo!();
        // self.output_map.iter()
    }
}

pub fn register(engine: &mut Engine) {
    let outputs_module = exported_module!(outputs);
    let modes_module = exported_module!(modes);
    engine
        .register_static_module("outputs", outputs_module.into())
        .register_static_module("modes", modes_module.into())
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
