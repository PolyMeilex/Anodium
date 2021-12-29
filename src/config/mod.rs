use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, FuncArgs, Scope, AST};

mod anodize;
pub mod eventloop;
pub mod keyboard;
mod log;
pub mod outputs;
mod system;
mod windows;
mod workspace;

use smithay::backend::input::KeyState;
use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::calloop::LoopHandle;

use crate::output_map::{Output, OutputMap};
use crate::state::Anodium;

use self::anodize::Anodize;
use self::eventloop::ConfigEvent;

#[derive(Debug)]
struct Inner {
    engine: Engine,
    ast: AST,
    #[allow(unused)]
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
        self.anodize.outputs.on_rearrange(&inner.engine, &inner.ast);
    }

    pub fn output_new(&self, output: Output) {
        let inner = &*self.inner.borrow();
        self.anodize
            .outputs
            .on_new(&inner.engine, &inner.ast, output);
    }
}
