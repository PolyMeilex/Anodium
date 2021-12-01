#[macro_use]
extern crate slog_scope;

mod input_handler;

mod framework;

mod grabs;

mod render;
mod utils;

mod animations;
mod positioner;

mod config;

mod output_map;

mod popup;
mod window;

mod state;

mod backend_handler;
mod shell_handler;

mod ipc;

use state::Anodium;

use slog::Drain;
use smithay::reexports::calloop::EventLoop;
use std::sync::atomic::Ordering;

fn main() {
    // A logger facility, here we use the terminal here
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        //std::sync::Mutex::new(slog_term::term_full().fuse()).fuse(),
        slog::o!(),
    );
    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let mut event_loop = EventLoop::try_new().unwrap();

    ipc::ipc_listener(event_loop.handle());

    let anodium = framework::backend::auto(&mut event_loop);
    let anodium = anodium.expect("Could not create a backend!");
    run_loop(anodium, event_loop);
}

fn run_loop(mut state: Anodium, mut event_loop: EventLoop<'static, Anodium>) {
    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut state, |state| {
            if state.output_map.is_empty() || !state.running.load(Ordering::SeqCst) {
                signal.stop();
            }

            state.display.borrow_mut().flush_clients(&mut ());
            state.update();
        })
        .unwrap();
}
