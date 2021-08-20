#[macro_use]
extern crate slog;

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

mod wayland;

use state::MainState;

static POSSIBLE_BACKENDS: &[&str] = &[
    #[cfg(feature = "winit")]
    "--winit : Run anvil as a X11 or Wayland client using winit.",
    #[cfg(feature = "udev")]
    "--tty-udev : Run anvil as a tty udev client (requires root if without logind).",
];

fn main() {
    // A logger facility, here we use the terminal here
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        //std::sync::Mutex::new(slog_term::term_full().fuse()).fuse(),
        o!(),
    );
    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        #[cfg(feature = "winit")]
        Some("--winit") => {
            backend::winit(log);
        }
        #[cfg(feature = "udev")]
        Some("--tty-udev") => {
            backend::udev(log);
        }
        Some(other) => {
            crit!(log, "Unknown backend: {}", other);

            println!("Possible backends are:");
            for b in POSSIBLE_BACKENDS {
                println!("\t{}", b);
            }
            println!("USAGE: anvil --backend");
            println!();
        }
        None => {
            backend::auto(log);
        }
    }
}
