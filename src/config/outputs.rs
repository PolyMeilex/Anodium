use std::cell::RefCell;
use std::rc::Rc;

use rhai::{plugin::*, AST};
use rhai::{FnPtr, INT};

use crate::output_map::{Output, OutputMap}; // a "prelude" import for macros

#[derive(Debug, Clone)]
pub struct Outputs {
    output_map: OutputMap,
    on_rearrange: Rc<RefCell<Option<FnPtr>>>,
}

impl Outputs {
    pub fn new(output_map: OutputMap) -> Self {
        Self {
            output_map,
            on_rearrange: Default::default(),
        }
    }

    pub fn rearrange(&self, engine: &Engine, ast: &AST) {
        if let Some(on_rearrange) = self.on_rearrange.borrow().clone() {
            let _result: () = on_rearrange.call(engine, ast, ()).unwrap();
        } else {
            error!("on_rearrange not configured");
        }
    }
}

#[export_module]
pub mod outputs {
    #[rhai_fn(get = "w", pure)]
    pub fn w(output: &mut Output) -> INT {
        output.geometry().size.w as _
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(output: &mut Output) -> INT {
        output.geometry().size.h as _
    }

    #[rhai_fn(get = "x", pure)]
    pub fn get_x(output: &mut Output) -> INT {
        output.geometry().loc.x as _
    }

    #[rhai_fn(get = "y", pure)]
    pub fn y(output: &mut Output) -> INT {
        output.geometry().loc.y as _
    }

    #[rhai_fn(set = "x", pure)]
    pub fn x(output: &mut Output, x: INT) {
        let mut location = output.location();
        location.x = x as _;
        output.set_location(location);
        let geometry = output.geometry();
        output.layer_map_mut().arange(geometry);
    }

    #[rhai_fn(global)]
    pub fn set_wallpaper(output: &mut Output, path: &str) {
        output.set_wallpaper(path);
    }

    #[rhai_fn(global)]
    pub fn on_rearrange(output: &mut Outputs, fnptr: FnPtr) {
        *output.on_rearrange.borrow_mut() = Some(fnptr);
    }
}

impl IntoIterator for Outputs {
    type Item = Output;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.output_map.iter()
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
