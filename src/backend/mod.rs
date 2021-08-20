#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "winit")]
pub fn winit(log: slog::Logger) {
    info!(log, "Starting anvil with winit backend");
    let mut event_loop = EventLoop::try_new().unwrap();
    let display = Rc::new(RefCell::new(Display::new()));
    if let Err(()) = winit::run_winit(display, &mut event_loop, log.clone()) {
        crit!(log, "Failed to initialize winit backend.");
    }
}

#[cfg(feature = "udev")]
pub fn udev(log: slog::Logger) {
    info!(log, "Starting anvil on a tty using udev");
    let mut event_loop = EventLoop::try_new().unwrap();
    let display = Rc::new(RefCell::new(Display::new()));
    if let Err(()) = udev::run_udev(display, &mut event_loop, log.clone()) {
        crit!(log, "Failed to initialize tty backend.");
    }
}

pub fn auto(log: slog::Logger) {
    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        winit(log);
    } else {
        #[cfg(feature = "udev")]
        udev(log);
    }
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn change_vt(&mut self, vt: i32);
}
