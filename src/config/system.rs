use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult, FnPtr};
use smithay::reexports::calloop::LoopHandle;

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::calloop::timer::{Timeout as TimeoutHandle, Timer, TimerHandle};

use crate::state::Anodium;

use super::eventloop::ConfigEvent;

#[derive(Debug, Clone)]
pub struct Timeout {
    fnptr: FnPtr,
    duration: Duration,
    timeout_handle: Rc<RefCell<Option<TimeoutHandle>>>,
}

impl Timeout {
    pub fn new(fnptr: FnPtr, millis: u64) -> Self {
        Self {
            fnptr: fnptr,
            duration: Duration::from_millis(millis),
            timeout_handle: Default::default(),
        }
    }

    pub fn set_timeout_handle(&self, timeout_handle: TimeoutHandle) {
        *self.timeout_handle.borrow_mut() = Some(timeout_handle);
    }

    pub fn get_timeout_handle(&self) -> Option<TimeoutHandle> {
        self.timeout_handle.borrow_mut().take()
    }
}

#[derive(Debug, Clone)]
pub struct System {
    #[allow(unused)]
    event_sender: Sender<ConfigEvent>,
    loop_handle: LoopHandle<'static, Anodium>,
    timer_handle: TimerHandle<Timeout>,
}

impl System {
    pub fn new(
        event_sender: Sender<ConfigEvent>,
        loop_handle: LoopHandle<'static, Anodium>,
    ) -> Self {
        let source: Timer<Timeout> = Timer::new().expect("Failed to create timer event source!");
        let timer_handle = source.handle();

        let system = Self {
            event_sender,
            loop_handle,
            timer_handle,
        };

        let share_system = system.clone();
        system
            .loop_handle
            .insert_source(source, move |timeout, _metadata, shared_data| {
                if let Ok(result) = shared_data
                    .config
                    .execute_fnptr(timeout.fnptr.clone(), ())
                    .as_bool()
                {
                    if result {
                        let timeout_handle = share_system
                            .timer_handle
                            .add_timeout(timeout.duration, timeout.clone());

                        timeout.set_timeout_handle(timeout_handle);
                    }
                }
            })
            .unwrap();

        system
    }
}

#[export_module]
pub mod system {
    #[rhai_fn(global)]
    pub fn exec(_system: &mut System, command: &str) {
        if let Err(e) = Command::new(command).spawn() {
            slog_scope::error!("failed to start command: {}, err: {:?}", command, e);
        }
    }

    #[rhai_fn(global)]
    pub fn add_timeout(system: &mut System, fnptr: FnPtr, millis: i64) -> Timeout {
        let millis = millis as u64;
        let timeout = Timeout::new(fnptr, millis);

        let timeout_handle = system
            .timer_handle
            .add_timeout(timeout.duration, timeout.clone());

        timeout.set_timeout_handle(timeout_handle);

        timeout
    }

    #[rhai_fn(global)]
    pub fn clear_timeout(system: &mut System, timeout: Dynamic) {
        if let Some(timeout) = timeout.try_cast::<Timeout>() {
            if let Some(timeout_handle) = timeout.get_timeout_handle() {
                system.timer_handle.cancel_timeout(&timeout_handle);
            }
        }
    }
}

pub fn register(engine: &mut Engine) {
    let system_module = exported_module!(system);

    engine
        .register_static_module("system", system_module.into())
        .register_type::<System>()
        .register_type::<Timeout>();
}
