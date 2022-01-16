#![allow(clippy::too_many_arguments)]
#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate slog_scope;

mod event_handler;
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

mod cli;

use config::outputs::shell::logger::ShellDrain;
use state::Anodium;

use slog::Drain;

use smithay::reexports::calloop::EventLoop;

use std::sync::atomic::Ordering;

fn main() {
    let options = cli::get_anodium_options();

    std::env::set_var("RUST_LOG", "trace,smithay=error");
    let terminal_drain = slog_async::Async::default(slog_envlogger::new(
        slog_term::CompactFormat::new(slog_term::TermDecorator::new().stderr().build())
            .build()
            .fuse(),
    ))
    .fuse();

    let shell_drain = slog_async::Async::default(ShellDrain::new().fuse());

    let log = slog::Logger::root(
        slog::Duplicate::new(terminal_drain, shell_drain).fuse(),
        //terminal_drain,
        //std::sync::Mutex::new(slog_term::term_full().fuse()).fuse(),
        slog::o!(),
    );
    let _guard = slog_scope::set_global_logger(log.clone());
    //let _guard = slog_envlogger::init().expect("Could not setup log backend");
    slog_stdlog::init().expect("Could not setup log backend");

    //let _guard = slog_envlogger::new(_log);

    //slog_scope::set_global_logger(_guard);

    let mut event_loop = EventLoop::try_new().unwrap();

    let anodium = framework::backend::auto(&mut event_loop, options);
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
