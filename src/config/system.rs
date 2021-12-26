use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult, FnPtr};
use smithay::reexports::calloop::LoopHandle;

use std::cell::RefCell;
use std::error::Error;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use futures::io::AsyncReadExt;

use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::calloop::timer::{Timeout as TimeoutHandle, Timer, TimerHandle};
use smithay::reexports::calloop::{futures as calloop_futures, futures::Scheduler};

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
    #[allow(unused)]
    io_scheduler: Rc<Scheduler<(FnPtr, Result<String, Box<dyn Error>>)>>,
}

impl System {
    pub fn new(
        event_sender: Sender<ConfigEvent>,
        loop_handle: LoopHandle<'static, Anodium>,
    ) -> Self {
        let timer_source: Timer<Timeout> =
            Timer::new().expect("Failed to create timer event source!");
        let timer_handle = timer_source.handle();

        let (io_source, io_scheduler) = calloop_futures::executor().unwrap();

        loop_handle
            .insert_source(
                io_source,
                |evt: (FnPtr, Result<String, Box<dyn Error>>), _metadata, shared_data| {
                    //TODO: execute call back on error too, pass informantion about that to callback
                    if let Ok(result) = evt.1 {
                        shared_data.config.execute_fnptr(evt.0.clone(), (result,));
                    }
                },
            )
            .unwrap();

        let system = Self {
            event_sender,
            loop_handle,
            timer_handle,
            io_scheduler: Rc::new(io_scheduler),
        };

        let share_system = system.clone();
        system
            .loop_handle
            .insert_source(timer_source, move |timeout, _metadata, shared_data| {
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
        let command_split = shell_words::split(&command).unwrap();

        if let Err(e) = Command::new(&command_split[0])
            .args(&command_split[1..])
            .spawn()
        {
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

    #[rhai_fn(global)]
    pub fn exec_read(system: &mut System, command: String, callback: FnPtr) {
        let command_split = shell_words::split(&command).unwrap();

        let loop_handle_io = system.loop_handle.clone();
        let async_spawn = async move {
            let mut child = Command::new(&command_split[0])
                .args(&command_split[1..])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;

            let stdout = child.stdout.take().unwrap();
            let mut reader = loop_handle_io.adapt_io(stdout)?;
            let mut readed = String::new();
            reader.read_to_string(&mut readed).await?;
            //TODO: return Err when no code or code is not zero
            let code = child.wait()?.code();

            Ok(readed)
        };

        let async_wrap = async move { (callback, async_spawn.await) };

        system.io_scheduler.schedule(async_wrap).unwrap();
    }
}

pub fn register(engine: &mut Engine) {
    let system_module = exported_module!(system);

    engine
        .register_static_module("system", system_module.into())
        .register_type::<System>()
        .register_type::<Timeout>();
}
