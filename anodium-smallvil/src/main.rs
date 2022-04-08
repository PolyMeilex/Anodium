use {
    anodium_framework::shell::ShellManager,
    smithay::reexports::calloop::{
        self, channel, generic::Generic, EventLoop, Interest, LoopSignal, PostAction,
    },
    smithay::wayland::{data_device, output::xdg::init_xdg_output_manager, shm::init_shm_global},
    smithay::{desktop, reexports::wayland_server::Display, wayland::seat::Seat},
    std::{cell::RefCell, rc::Rc, time::Instant},
};

mod backend_handler;
mod input_handler;
mod output_handler;
mod shell_handler;

struct State {
    space: desktop::Space,
    display: Rc<RefCell<Display>>,
    seat: Seat,
    shell_manager: ShellManager<Self>,

    start_time: Instant,
    loop_signal: LoopSignal,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<State>::try_new()?;
    let display = Rc::new(RefCell::new(Display::new()));

    init_shm_global(&mut display.borrow_mut(), vec![], None);
    init_xdg_output_manager(&mut display.borrow_mut(), None);

    data_device::init_data_device(
        &mut display.borrow_mut(),
        |_| {},
        data_device::default_action_chooser,
        None,
    );

    let shell_manager = ShellManager::init_shell(&mut display.borrow_mut());

    let (mut seat, _) = Seat::new(&mut display.borrow_mut(), "seat0".into(), None);

    seat.add_pointer(|_| {});
    seat.add_keyboard(Default::default(), 200, 25, |seat, focus| {
        data_device::set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
    })?;

    let mut state = State {
        space: desktop::Space::new(None),
        display: display.clone(),
        shell_manager,
        seat,
        start_time: Instant::now(),
        loop_signal: event_loop.get_signal(),
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

    let (_backend_tx, backend_rx) = channel::channel();

    anodium_backend::init(
        &mut event_loop,
        display,
        &mut state,
        backend_rx,
        anodium_backend::PreferedBackend::Auto,
    );

    event_loop.run(None, &mut state, |state| {
        state.shell_manager.refresh();
        state.space.refresh();
        state.display.borrow_mut().flush_clients(&mut ());
    })?;

    Ok(())
}