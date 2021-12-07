use std::collections::HashSet;
use std::rc::Rc;
use std::{cell::RefCell, convert::TryInto};

use rhai::{
    Array, Dynamic, Engine, EvalAltResult, FnPtr, FuncArgs, ImmutableString, Module,
    NativeCallContext, Position, Scope, AST,
};

mod anodize;
pub mod eventloop;
pub mod keyboard;
mod log;
mod output;
mod system;
mod windows;
mod workspace;

use output::OutputConfig;
use smithay::backend::input::KeyState;
use smithay::reexports::{calloop::channel::Sender, drm};

use self::anodize::Anodize;
use self::{
    eventloop::{ConfigEvent, EventLoop},
    output::Mode,
};

#[derive(Debug, Clone)]
pub struct FnCallback {
    fn_ptr: FnPtr,
    fn_name: String,
    lib: Box<[Module]>,
}

impl FnCallback {
    pub fn new(fn_ptr: FnPtr, context: NativeCallContext) -> Self {
        Self {
            fn_ptr,
            fn_name: context.fn_name().to_owned(),
            lib: context
                .iter_namespaces()
                .map(|x| x.clone().clone())
                .collect::<Vec<Module>>()
                .into_boxed_slice(),
        }
    }

    pub fn call(
        &self,
        engine: &Engine,
        this_ptr: Option<&mut Dynamic>,
        args: &mut [Dynamic],
    ) -> Dynamic {
        let lib = self.lib.iter().map(|x| x).collect::<Vec<&Module>>();

        let context = NativeCallContext::new(engine, &self.fn_name, &lib[..]);

        self.fn_ptr.call_dynamic(&context, this_ptr, args).unwrap()
    }
}

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
        workspace::register(&mut engine);
        windows::register(&mut engine);

        let anodize = anodize::register(&mut scope, &mut engine, event_sender.clone());

        let ast = engine.compile_file("config.rhai".into())?;

        engine.eval_ast_with_scope(&mut scope, &ast)?;

        Ok(ConfigVM {
            inner: Rc::new(RefCell::new(Inner { engine, ast, scope })),
            anodize,
            event_sender,
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
    }

    pub fn execute_callback(&self, callback: FnCallback, args: &mut [Dynamic]) -> Dynamic {
        let inner = &mut *self.inner.borrow_mut();
        callback.call(&inner.engine, None, args)
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
}
