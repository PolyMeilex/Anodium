use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, FuncArgs, ImmutableString, Scope, AST};

pub mod keyboard;
mod log;
mod output;
mod state;
mod system;

use output::OutputConfig;
use smithay::reexports::drm;

use crate::desktop_layout::{self, DesktopLayout};

use self::{output::Mode, state::StateConfig};

#[derive(Debug)]
struct Inner {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
}

#[derive(Debug, Clone)]
pub struct ConfigVM {
    inner: Rc<RefCell<Inner>>,
    desktop_layout: Option<Rc<RefCell<DesktopLayout>>>,
}

impl ConfigVM {
    pub fn new() -> Result<ConfigVM, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        let mut scope = Scope::new();

        engine.register_fn("rev", |array: Array| {
            array.into_iter().rev().collect::<Array>()
        });

        output::register(&mut engine);

        keyboard::register(&mut engine);
        log::register(&mut engine);
        system::register(&mut engine);
        state::register(&mut engine);

        let ast = engine.compile_file("config.rhai".into())?;

        keyboard::callbacks_clear();

        engine.eval_ast_with_scope(&mut scope, &ast)?;

        Ok(ConfigVM {
            inner: Rc::new(RefCell::new(Inner { engine, ast, scope })),
            desktop_layout: None,
        })
    }

    pub fn arrange_outputs(
        &mut self,
        outputs: &[desktop_layout::Output],
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

    pub fn execute_fn_with_state(&self, function: &str, state: StateConfig) {
        let inner = &mut *self.inner.borrow_mut();
        let mut state: Dynamic = state.into();
        inner
            .engine
            .call_fn_dynamic(
                &mut inner.scope,
                &inner.ast,
                false,
                function.to_string(),
                Some(&mut state),
                [],
            )
            .unwrap();

        //let result: Dynamic = inner
        //    .engine
        //    .call_fn(&mut inner.scope, &inner.ast, function, (state,))
        //    .unwrap();
    }

    //HACK: workaround currnect check and eg problem betwenn DesktopLayout and ConfigVM
    pub fn set_desktop_layout(&mut self, desktop_layout: Rc<RefCell<DesktopLayout>>) {
        self.desktop_layout = Some(desktop_layout);
    }

    //HACK: workaround currnect check and eg problem betwenn DesktopLayout and ConfigVM
    pub fn get_desktop_layout(&self) -> Rc<RefCell<DesktopLayout>> {
        self.desktop_layout
            .as_ref()
            .expect("desktop layout not set in configvm")
            .clone()
    }
}
