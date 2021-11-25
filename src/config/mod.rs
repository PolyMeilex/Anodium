use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, FuncArgs, ImmutableString, Scope, AST};

pub mod eventloop;
pub mod keyboard;
mod log;
mod output;
mod system;

use output::OutputConfig;
use smithay::reexports::{calloop::channel::Sender, drm};

use self::{
    eventloop::{ConfigEvent, EventLoop},
    output::Mode,
};

#[derive(Debug)]
struct Inner {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
}

#[derive(Debug, Clone)]
pub struct ConfigVM {
    inner: Rc<RefCell<Inner>>,
}

impl ConfigVM {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Result<ConfigVM, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        let mut scope = Scope::new();

        engine.register_fn("rev", |array: Array| {
            array.into_iter().rev().collect::<Array>()
        });

        output::register(&mut engine);

        keyboard::register(&mut engine);
        log::register(&mut engine);
        system::register(&mut engine);
        eventloop::register(&mut scope, &mut engine, event_sender);

        let ast = engine.compile_file("config.rhai".into())?;

        keyboard::callbacks_clear();

        engine.eval_ast_with_scope(&mut scope, &ast)?;

        Ok(ConfigVM {
            inner: Rc::new(RefCell::new(Inner { engine, ast, scope })),
        })
    }

    pub fn arrange_outputs(
        &mut self,
        outputs: &[crate::output_map::Output],
    ) -> Result<Vec<OutputConfig>, Box<EvalAltResult>> {
        let inner = &mut *self.inner.borrow_mut();

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
        let inner = &mut *self.inner.borrow_mut();

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

    pub fn execute_fn_with_state(&self, function: &str, args: &mut [Dynamic]) {
        let inner = &mut *self.inner.borrow_mut();
        let event_loop = inner.scope.get_value::<EventLoop>("_event_loop").unwrap();

        inner
            .engine
            .call_fn_dynamic(
                &mut inner.scope,
                &inner.ast,
                false,
                function.to_string(),
                Some(&mut event_loop.into()),
                args,
            )
            .unwrap();
    }
}
