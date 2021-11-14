use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, ImmutableString, Scope, AST};

mod output;
use output::OutputConfig;
use smithay::reexports::drm;

use crate::desktop_layout;

use self::output::Mode;

#[derive(Debug)]
struct Inner {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
}

#[derive(Debug, Clone)]
pub struct ConfigVM(Rc<RefCell<Inner>>);

impl ConfigVM {
    pub fn new() -> Result<ConfigVM, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        let ast = engine.compile(include_str!("../config.rhai"))?;
        let scope = Scope::new();

        engine.register_fn("rev", |array: Array| {
            array.into_iter().rev().collect::<Array>()
        });

        output::register(&mut engine);

        Ok(ConfigVM(Rc::new(RefCell::new(Inner {
            engine,
            ast,
            scope,
        }))))
    }

    pub fn arrange_outputs(
        &mut self,
        outputs: &[desktop_layout::Output],
    ) -> Result<Vec<OutputConfig>, Box<EvalAltResult>> {
        let inner = &mut *self.0.borrow_mut();

        let outputs: Array = outputs
            .iter()
            .enumerate()
            .map(|(id, o)| {
                let location = o.location();
                let size = o.size();

                Dynamic::from(OutputConfig {
                    id,
                    name: o.name().into(),
                    x: location.x as _,
                    y: location.y as _,
                    w: size.w as _,
                    h: size.h as _,
                })
            })
            .collect();

        let result: Array =
            inner
                .engine
                .call_fn(&mut inner.scope, &inner.ast, "arrange_outputs", (outputs,))?;

        Ok(result
            .into_iter()
            .map(|item| item.try_cast().unwrap())
            .collect())
    }

    pub fn configure_output(
        &mut self,
        output_name: &str,
        modes: &[drm::control::Mode],
    ) -> Result<usize, Box<EvalAltResult>> {
        let inner = &mut *self.0.borrow_mut();

        let modes: Array = modes
            .iter()
            .enumerate()
            .map(|(id, m)| Dynamic::from(Mode { id, mode: *m }))
            .collect();

        let output_name: ImmutableString = output_name.into();

        let result: Dynamic = inner.engine.call_fn(
            &mut inner.scope,
            &inner.ast,
            "configure_output",
            (output_name, modes),
        )?;

        let mode: Option<Mode> = result.try_cast();
        let id = mode.map(|m| m.id).unwrap_or(0);
        Ok(id)
    }
}
