use std::cell::RefCell;
use std::rc::Rc;

use rhai::{
    Array, Dynamic, Engine, EvalAltResult, FnPtr, FuncArgs, ImmutableString, Imports, Module,
    NativeCallContext, Position, Scope, AST,
};

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

#[derive(Debug, Clone)]
pub struct NativeCallContextWraper {
    fn_name: String,
    position: Position,
    source: Option<String>,
    imports: Option<Imports>,
    lib: Box<[Module]>,
}

impl NativeCallContextWraper {
    pub fn new(context: NativeCallContext) -> Self {
        let fn_name = context.fn_name().to_owned();
        let position = context.position();
        let mut source = None;
        if let Some(source_copy) = context.source() {
            source = Some(source_copy.to_owned());
        }

        let mut imports = None;
        if let Some(imports_copy) = context.imports() {
            imports = Some(imports_copy.clone());
        }

        let mut lib = Vec::with_capacity(context.namespaces().len());
        for namespace in context.namespaces().iter() {
            lib.push(namespace.clone().clone());
        }

        Self {
            fn_name,
            position,
            source,
            imports,
            lib: lib.into_boxed_slice(),
        }
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

    pub fn execute_fnptr_with_state(
        &self,
        function: FnPtr,
        contexted_wraped: NativeCallContextWraper,
        args: &mut [Dynamic],
    ) -> Dynamic {
        let inner = &mut *self.inner.borrow_mut();
        let event_loop = inner.scope.get_value::<EventLoop>("_event_loop").unwrap();
        //let fnptr = inner.ast.iter_fn_def();

        let lib = contexted_wraped
            .lib
            .iter()
            .map(|x| x)
            .collect::<Vec<&Module>>();
        let context = NativeCallContext::new_with_all_fields(
            &inner.engine,
            &contexted_wraped.fn_name,
            contexted_wraped.source.as_ref().map(|x| &x[..]),
            &contexted_wraped.imports.as_ref().unwrap(),
            &lib[..],
            contexted_wraped.position,
        );

        info!("context: {:?}", context);
        function
            .call_dynamic(&context, Some(&mut event_loop.into()), args)
            .unwrap()
    }

    pub fn execute_fn_with_state(&self, function: &str, args: &mut [Dynamic]) -> Dynamic {
        let inner = &mut *self.inner.borrow_mut();
        //let fnptr = inner.ast.iter_fn_def();
        let function_metadata = inner
            .ast
            .iter_functions()
            .find(|x| x.name == function)
            .unwrap();

        let curried_arguments = function_metadata.params;
        info!("curried_arguments: {:?}", curried_arguments);
        let mut args = Vec::with_capacity(curried_arguments.len());
        for curried in curried_arguments {
            let dynamic: Dynamic = inner.scope.get_value(curried).unwrap();
            let dynamic = dynamic.into_shared();
            inner.scope.push(curried.to_string(), dynamic.clone());
            args.push(dynamic);
        }

        let event_loop = inner.scope.get_value::<EventLoop>("_event_loop").unwrap();
        inner
            .engine
            .call_fn_raw(
                &mut inner.scope,
                &inner.ast,
                false,
                true,
                function,
                Some(&mut event_loop.into()),
                args,
            )
            .unwrap()
    }

    pub fn insert_event(&self, event: ConfigEvent) {
        let inner = &mut *self.inner.borrow_mut();
        let event_loop = inner.scope.get_value::<EventLoop>("_event_loop").unwrap();
        event_loop.0.send(event).unwrap();
    }
}
