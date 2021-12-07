use rhai::{plugin::*, Scope};
use rhai::{Dynamic, EvalAltResult, FnPtr};

use smithay::reexports::calloop::channel::Sender;

use super::FnCallback;

#[derive(Debug)]
pub enum ConfigEvent {
    CloseFocused,
    MaximizeFocused,
    UnmaximizeFocused,
    SwitchWorkspace(String),
    Timeout(FnCallback, u64),
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
}

pub fn register(scope: &mut Scope, engine: &mut Engine, event_sender: Sender<ConfigEvent>) {
    let module = exported_module!(event_loop_module);

    engine
        .register_type::<EventLoop>()
        .register_global_module(module.into());

    scope.push("_event_loop", EventLoop::new(event_sender));
}
