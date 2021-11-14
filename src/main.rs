#[macro_use]
extern crate slog_scope;

use slog::Drain;

#[cfg(feature = "udev")]
mod cursor;
mod input_handler;
mod shell;
mod state;

mod backend;

mod render;
mod utils;

#[cfg(feature = "xwayland")]
mod xwayland;

mod animations;
mod desktop_layout;
mod positioner;

mod config;

use state::Anodium;

fn main() {
    // A logger facility, here we use the terminal here
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        //std::sync::Mutex::new(slog_term::term_full().fuse()).fuse(),
        slog::o!(),
    );
    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    backend::auto(log);
}
