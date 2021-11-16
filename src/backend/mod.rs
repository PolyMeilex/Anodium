#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub mod session;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use std::sync::atomic::Ordering;
use std::{cell::RefCell, rc::Rc};

use crate::desktop_layout::Output;
use crate::state::BackendState;

pub enum BackendEvent {
    OutputCreated { output: Output },
}

#[cfg(feature = "winit")]
pub fn winit(
    _log: slog::Logger,
    event_loop: &mut EventLoop<'static, BackendState>,
) -> BackendState {
    use crate::backend::session::AnodiumSession;

    info!("Starting Anodium with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let mut state = BackendState::init(
        display.clone(),
        event_loop.handle(),
        AnodiumSession::new_winit(),
        slog_scope::logger(),
    );

    winit::run_winit(
        display,
        &mut state,
        event_loop,
        |event, mut ddata| match event {
            BackendEvent::OutputCreated { output } => {
                let state = ddata.get::<BackendState>().unwrap();
                state.anodium.add_output(output, |_| {});
            }
        },
    )
    .expect("Failed to initialize winit backend.");

    info!("Winit initialized");

    state
}

#[cfg(feature = "udev")]
pub fn udev(log: slog::Logger, event_loop: &mut EventLoop<'static, BackendState>) -> BackendState {
    info!("Starting Anodium on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    if let Ok(state) = udev::run_udev(display, event_loop, |event, mut ddata| match event {
        BackendEvent::OutputCreated { output } => {
            let state = ddata.get::<BackendState>().unwrap();
            state.anodium.add_output(output, |_| {});
        }
    }) {
        state
    } else {
        panic!("Failed to initialize tty backend.");
    }
}

pub fn auto(log: slog::Logger) {
    let mut event_loop = EventLoop::try_new().unwrap();

    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        {
            let state = winit(log, &mut event_loop);
            run_loop(state, event_loop)
        }
    } else {
        #[cfg(feature = "udev")]
        {
            let state = udev(log, &mut event_loop);
            run_loop(state, event_loop)
        }
    }
}

fn run_loop(mut state: BackendState, mut event_loop: EventLoop<'static, BackendState>) {
    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut state, |state| {
            if state.anodium.desktop_layout.borrow().output_map.is_empty()
                || !state.anodium.running.load(Ordering::SeqCst)
            {
                signal.stop();
            }

            state.anodium.display.borrow_mut().flush_clients(&mut ());
            state.anodium.update();
        })
        .unwrap();
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn change_vt(&mut self, vt: i32);
}
