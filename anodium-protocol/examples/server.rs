use std::{cell::RefCell, error::Error, rc::Rc, time::Duration};

use anodium_protocol::server::AnodiumProtocol;

use calloop::{EventLoop, LoopHandle};
use wayland_server::Display;

struct State {
    display: Rc<RefCell<Display>>,
    anodium_protocol: AnodiumProtocol,
}

fn insert_wayland_source(
    handle: LoopHandle<'static, State>,
    display: &Display,
) -> Result<(), Box<dyn Error>> {
    handle.insert_source(
        calloop::generic::Generic::from_fd(
            display.get_poll_fd(), // The file descriptor which indicates there are pending messages
            calloop::Interest::READ,
            calloop::Mode::Level,
        ),
        // This callback is invoked when the poll file descriptor has had activity, indicating there are pending messages.
        move |_, _, state: &mut State| {
            let display = state.display.clone();
            let mut display = display.borrow_mut();
            // Display::dispatch will process any queued up requests and send those events to any objects created on the server.
            display.dispatch(Duration::from_millis(0), state).unwrap();

            Ok(calloop::PostAction::Continue)
        },
    )?;
    Ok(())
}

fn main() {
    let mut display = Display::new();
    let socket_name = "wayland-0";

    display
        .add_socket(Some(socket_name))
        .expect("Failed to add wayland socket");

    println!("Listening on wayland socket {}", socket_name);

    let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();

    insert_wayland_source(event_loop.handle(), &display).unwrap();

    let (mut anodium_protocol, _) = AnodiumProtocol::init(&mut display);

    let mut out1 = anodium_protocol.new_output();
    out1.set_name("HDMI-1");

    let display = Rc::new(RefCell::new(display));

    let timer = calloop::timer::Timer::new().unwrap();
    let handle = timer.handle();

    event_loop
        .handle()
        .insert_source(timer, {
            let handle = handle.clone();
            move |count, _, state| {
                let mut output = state.anodium_protocol.new_output();
                output.set_name(format!("HDMI-{}", count));

                handle.add_timeout(Duration::from_secs(1), count + 1);
            }
        })
        .unwrap();

    handle.add_timeout(Duration::from_secs(1), 0);

    event_loop
        .run(
            None,
            &mut State {
                display,
                anodium_protocol,
            },
            |state| {
                state.display.clone().borrow_mut().flush_clients(state);
            },
        )
        .unwrap();
}
