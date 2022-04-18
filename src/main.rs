#![allow(irrefutable_let_patterns)]

use anodium_backend::BackendState;
use anodium_framework::{pointer_icon::PointerIcon, shell::ShellManager};

use clap::StructOpt;
use config::ConfigVM;
use slog::Drain;
use smithay::{
    desktop,
    reexports::{
        calloop::{self, generic::Generic, EventLoop, Interest, LoopSignal, PostAction},
        wayland_server::Display,
    },
    wayland::{
        data_device::{self},
        output::xdg::init_xdg_output_manager,
        seat::Seat,
        shm::init_shm_global,
    },
};

use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

#[cfg(feature = "xwayland")]
use smithay::{reexports::calloop::LoopHandle, xwayland::XWayland};

mod cli;
mod config;
mod handlers;

struct State {
    space: desktop::Space,
    display: Rc<RefCell<Display>>,

    seat: Seat,
    pressed_keys: HashSet<u32>,

    shell_manager: ShellManager<Self>,

    start_time: Instant,
    loop_signal: LoopSignal,

    pointer_icon: PointerIcon,

    backend: BackendState,

    config: ConfigVM,

    #[cfg(feature = "xwayland")]
    xwayland: XWayland<Self>,
}

/// init the xwayland connection
#[cfg(feature = "xwayland")]
fn init_xwayland_connection(
    handle: &LoopHandle<'static, State>,
    display: &Rc<RefCell<Display>>,
) -> XWayland<State> {
    use smithay::xwayland::XWaylandEvent;

    let (xwayland, channel) = XWayland::new(handle.clone(), display.clone(), None);

    handle
        .insert_source(channel, {
            let handle = handle.clone();
            move |event, _, state| match event {
                XWaylandEvent::Ready { connection, client } => state
                    .shell_manager
                    .xwayland_ready(&handle, connection, client),
                XWaylandEvent::Exited => {}
            }
        })
        .unwrap();

    xwayland
}

fn init_log() -> slog::Logger {
    std::env::set_var("RUST_LOG", "trace,smithay=error");

    let terminal_drain = slog_async::Async::default(slog_envlogger::new(
        slog_term::CompactFormat::new(slog_term::TermDecorator::new().stderr().build())
            .build()
            .fuse(),
    ))
    .fuse();

    let log = slog::Logger::root(terminal_drain.fuse(), slog::o!());

    slog_stdlog::init().expect("Could not setup log backend");

    log
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log = init_log();
    let _guard = slog_scope::set_global_logger(log.clone());

    let opt = cli::AnodiumCliOptions::parse();

    let config = ConfigVM::new(opt.config)?;

    let mut event_loop = EventLoop::<State>::try_new()?;
    let display = Rc::new(RefCell::new(Display::new()));

    init_shm_global(&mut display.borrow_mut(), vec![], None);
    init_xdg_output_manager(&mut display.borrow_mut(), None);

    let pointer_icon = PointerIcon::new();

    data_device::init_data_device(
        &mut display.borrow_mut(),
        {
            let pointer_icon = pointer_icon.clone();
            move |event| pointer_icon.on_data_device_event(event)
        },
        data_device::default_action_chooser,
        None,
    );

    let shell_manager = ShellManager::init_shell(&mut display.borrow_mut());

    let (mut seat, _) = Seat::new(
        &mut display.borrow_mut(),
        "seat0".into(),
        slog_scope::logger(),
    );

    seat.add_pointer({
        let pointer_icon = pointer_icon.clone();
        move |cursor| pointer_icon.on_new_cursor(cursor)
    });
    seat.add_keyboard(Default::default(), 200, 25, |seat, focus| {
        data_device::set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
    })?;

    let xwayland = init_xwayland_connection(&event_loop.handle(), &display);

    let mut state = State {
        space: desktop::Space::new(None),
        display: display.clone(),
        shell_manager,

        seat,
        pressed_keys: Default::default(),

        start_time: Instant::now(),
        loop_signal: event_loop.get_signal(),

        pointer_icon,
        backend: BackendState::default(),

        config,

        xwayland,
    };

    event_loop
        .handle()
        .insert_source(
            Generic::from_fd(
                display.borrow().get_poll_fd(),
                Interest::READ,
                calloop::Mode::Level,
            ),
            |_, _, state| {
                let display = state.display.clone();
                let mut display = display.borrow_mut();
                match display.dispatch(std::time::Duration::from_millis(0), state) {
                    Ok(_) => Ok(PostAction::Continue),
                    Err(e) => {
                        state.loop_signal.stop();
                        Err(e)
                    }
                }
            },
        )
        .expect("Failed to init the wayland event source.");

    anodium_backend::init(
        &mut event_loop,
        display,
        &mut state,
        anodium_backend::PreferedBackend::Auto,
    );

    event_loop.run(None, &mut state, |state| {
        state.shell_manager.refresh();
        state.space.refresh();
        state.display.borrow_mut().flush_clients(&mut ());
    })?;

    Ok(())
}
