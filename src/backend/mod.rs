#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use std::sync::atomic::Ordering;
use std::{cell::RefCell, rc::Rc};

use crate::state::BackendState;

#[cfg(feature = "winit")]
pub fn winit(
    log: slog::Logger,
    event_loop: &mut EventLoop<'static, BackendState<winit::WinitData>>,
) -> BackendState<winit::WinitData> {
    info!(log, "Starting anvil with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    if let Ok(state) = winit::run_winit(display, event_loop, log.clone()) {
        state
    } else {
        panic!("Failed to initialize winit backend.");
    }
}

#[cfg(feature = "udev")]
pub fn udev(
    log: slog::Logger,
    event_loop: &mut EventLoop<'static, BackendState<udev::UdevData>>,
) -> BackendState<udev::UdevData> {
    info!(log, "Starting anvil on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    if let Ok(state) = udev::run_udev(display, event_loop, log.clone()) {
        state
    } else {
        panic!("Failed to initialize tty backend.");
    }
}

pub fn auto(log: slog::Logger) {
    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        {
            let mut event_loop = EventLoop::try_new().unwrap();
            let state = winit(log, &mut event_loop);
            run_loop(state, event_loop)
        }
    } else {
        #[cfg(feature = "udev")]
        {
            let mut event_loop = EventLoop::try_new().unwrap();
            let state = udev(log, &mut event_loop);
            run_loop(state, event_loop)
        }
    }
}

fn run_loop<D>(mut state: BackendState<D>, mut event_loop: EventLoop<'static, BackendState<D>>) {
    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut state, |state| {
            if state.main_state.desktop_layout.borrow().output_map.is_empty()
                || !state.main_state.running.load(Ordering::SeqCst)
            {
                signal.stop();
            }
        })
        .unwrap();
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn change_vt(&mut self, vt: i32);
}
