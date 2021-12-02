use rhai::{plugin::*, Imports, Scope};
use rhai::{Dynamic, EvalAltResult, FnPtr};

use smithay::reexports::calloop::channel::Sender;

use super::NativeCallContextWraper;

#[derive(Debug)]
pub enum ConfigEvent {
    CloseFocused,
    MaximizeFocused,
    UnmaximizeFocused,
    SwitchWorkspace(String),
    Timeout(FnPtr, NativeCallContextWraper, String, u64),
}

#[derive(Debug, Clone)]
pub struct EventLoop(pub Sender<ConfigEvent>);

impl EventLoop {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self(event_sender)
    }
}

impl From<EventLoop> for Dynamic {
    fn from(event_loop: EventLoop) -> Self {
        rhai::Dynamic::from(event_loop)
    }
}

#[export_module]
pub mod event_loop_module {
    use input::Context;

    use super::EventLoop;

    pub fn focused_close(event_loop: &mut EventLoop) {
        event_loop.0.send(ConfigEvent::CloseFocused).unwrap();
    }

    pub fn focused_maximize(event_loop: &mut EventLoop) {
        event_loop.0.send(ConfigEvent::MaximizeFocused).unwrap();
    }

    pub fn focused_unmaximize(event_loop: &mut EventLoop) {
        event_loop.0.send(ConfigEvent::UnmaximizeFocused).unwrap();
    }

    pub fn switch_workspace(event_loop: &mut EventLoop, workspace: String) {
        event_loop
            .0
            .send(ConfigEvent::SwitchWorkspace(workspace))
            .unwrap();
    }

    pub fn add_timeout(
        context: NativeCallContext,
        event_loop: &mut EventLoop,
        callback: FnPtr,
        milis: i64,
    ) {
        if milis >= 0 {
            let callback_name = callback.fn_name().to_string();
            let fn_name = context.fn_name();
            info!("callback_name: {} fn_name: {}", callback_name, fn_name);
            info!("context: {:?}", context);

            let context_wraped = NativeCallContextWraper::new(context);

            event_loop
                .0
                .send(ConfigEvent::Timeout(
                    callback,
                    context_wraped,
                    callback_name,
                    milis as u64,
                ))
                .unwrap();
        }
    }
}

pub fn register(scope: &mut Scope, engine: &mut Engine, event_sender: Sender<ConfigEvent>) {
    let module = exported_module!(event_loop_module);

    engine
        .register_type::<EventLoop>()
        .register_global_module(module.into());

    scope.push("_event_loop", EventLoop::new(event_sender));
}
