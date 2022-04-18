use rhai::{Dynamic, Engine, EvalAltResult, Scope};
use smithay::backend::input::KeyState;
use smithay::wayland::seat::{KeysymHandle, ModifiersState};
use std::path::PathBuf;

mod keyboard;
mod log;
mod outputs;
mod system;

#[derive(Debug)]
pub struct ConfigVM {
    engine: Engine,
    ast: rhai::AST,
    keyboard: keyboard::Keyboard,
    outputs: outputs::Outputs,

    _scope: Scope<'static>,
}

impl ConfigVM {
    pub fn new(config: PathBuf) -> Result<ConfigVM, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(0, 0);
        let mut scope = Scope::new();

        log::register(&mut engine, &mut scope, slog_scope::logger());
        system::register(&mut engine, &mut scope);
        let outputs = outputs::register(&mut engine, &mut scope);
        let keyboard = keyboard::register(&mut engine, &mut scope);

        let ast = engine.compile_file(config)?;

        let _ignore: Dynamic = engine.eval_ast_with_scope(&mut scope, &ast)?;

        Ok(ConfigVM {
            engine,
            _scope: scope,
            ast,
            keyboard,
            outputs,
        })
    }

    pub fn key_action(
        &mut self,
        modifiers: &ModifiersState,
        keysym: &KeysymHandle,
        state: KeyState,
    ) {
        self.keyboard.callbacks.key_action(
            &mut self.engine,
            &mut self.ast,
            (*modifiers).into(),
            keysym,
            state,
        );
    }

    pub fn output_layout(&self) -> outputs::OutputLayout {
        self.outputs.layout()
    }
}
