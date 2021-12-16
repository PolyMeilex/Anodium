use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rhai::{
    Array, Dynamic, Engine, EvalAltResult, FnPtr, FuncArgs, ImmutableString, Module, Position,
    Scope, AST,
};

mod anodize;
pub mod eventloop;
pub mod keyboard;
mod log;
mod output;
mod outputs;
mod system;
mod windows;
mod workspace;

use output::OutputConfig;
use smithay::backend::input::KeyState;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::{calloop::channel::Sender, drm};

use crate::output_map::OutputMap;
use crate::state::Anodium;

use self::anodize::Anodize;
use self::{eventloop::ConfigEvent, output::Mode};

#[derive(Debug)]
struct Inner {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
}

#[derive(Debug, Clone)]
pub struct ConfigVM {
    inner: Rc<RefCell<Inner>>,
    pub anodize: Anodize,
    pub event_sender: Sender<ConfigEvent>,
}

impl ConfigVM {
    pub fn new(
        event_sender: Sender<ConfigEvent>,
        output_map: OutputMap,
        loop_handle: LoopHandle<'static, Anodium>,
    ) -> Result<ConfigVM, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(0, 0);
        let mut scope = Scope::new();

        //engine.register_fn("rev", |array: Array| {
        //    array.into_iter().rev().collect::<Array>()
        //});

        output::register(&mut engine);
        keyboard::register(&mut engine);
        log::register(&mut engine);
        system::register(&mut engine);
        workspace::register(&mut engine);
        windows::register(&mut engine);
        outputs::register(&mut engine);

        let anodize = anodize::register(
            &mut scope,
            &mut engine,
            event_sender.clone(),
            output_map,
            loop_handle,
        );

        let ast = engine.compile_file("config.rhai".into())?;

        engine.eval_ast_with_scope(&mut scope, &ast)?;

        Ok(ConfigVM {
            inner: Rc::new(RefCell::new(Inner { engine, ast, scope })),
            anodize,
            event_sender,
        })
    }

    /*pub fn arrange_outputs(
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

        let result: Array = inner
            .engine
            .call_fn_raw(
                &mut inner.scope,
                &inner.ast,
                false,
                true,
                "arrange_outputs",
                None,
                &mut [outputs.into()],
            )?
            .try_cast()
            .ok_or(EvalAltResult::ErrorMismatchOutputType(
                "".to_owned(),
                "".to_owned(),
                Position::NONE,
            ))?;

        Ok(result
            .into_iter()
            .map(|item| item.try_cast().unwrap())
            .collect())
    }*/

    /*pub fn configure_output(
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

        let result: Dynamic = inner.engine.call_fn_raw(
            &mut inner.scope,
            &inner.ast,
            false,
            true,
            "configure_output",
            None,
            &mut [output_name.into(), modes.into()],
        )?;

        let mode: Option<Mode> = result.try_cast();
        let id = mode.map(|m| m.id).unwrap_or(0);
        Ok(id)
    }*/

    pub fn execute_fnptr(&self, callback: FnPtr, args: impl FuncArgs) -> Dynamic {
        let inner = &mut *self.inner.borrow_mut();
        callback.call(&inner.engine, &inner.ast, args).unwrap()
    }

    pub fn insert_event(&self, event: ConfigEvent) {
        self.event_sender.send(event).unwrap();
    }

    pub fn key_action(&self, key: u32, state: KeyState, keys_pressed: &HashSet<u32>) -> bool {
        self.anodize
            .keyboard
            .callbacks
            .key_action(self, key, state, keys_pressed)
    }

    pub fn output_rearrange(&self) {
        let inner = &*self.inner.borrow();
        self.anodize.outputs.rearrange(&inner.engine, &inner.ast);
    }
}
