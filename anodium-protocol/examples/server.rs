use std::{cell::RefCell, error::Error, ops::Deref, rc::Rc, time::Duration};

use anodium_protocol::server::{
    anodium_output::AnodiumOutput, anodium_workspace::AnodiumWorkspace,
    anodium_workspace_manager::AnodiumWorkspaceManager,
};

use calloop::{EventLoop, LoopHandle};
use wayland_server::{Display, Filter, Global, Main};

struct State {
    display: Rc<RefCell<Display>>,
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

thread_local! {
    pub static FOO: RefCell<Vec<AnodiumWorkspace>> = Default::default();
}

fn init(display: &mut Display) {
    let global: Global<AnodiumWorkspaceManager> = display.create_global(
        1,
        Filter::new(|(res, _): (Main<AnodiumWorkspaceManager>, _), _, _| {
            println!("New Global");

            res.quick_assign(|_res, _, _| {
                println!("Assign");
            });

            let client = res.as_ref().client().unwrap();

            for id in 0..2 {
                let output: Main<AnodiumOutput> = client.create_resource(1).unwrap();
                output.quick_assign(|_res, _, _| {});

                res.output(output.deref());
                output.name(format!("HDMI-A-{}", id));

                {
                    let workspace: Main<AnodiumWorkspace> = client.create_resource(1).unwrap();
                    workspace.quick_assign(|_res, _, _| {});

                    output.workspace(&workspace);
                    workspace.name("Web".into());
                }

                {
                    let workspace: Main<AnodiumWorkspace> = client.create_resource(1).unwrap();
                    workspace.quick_assign(|_res, _, _| {});

                    output.workspace(&workspace);
                    workspace.name("Mes".into());

                    FOO.with(|data| {
                        data.borrow_mut().push(workspace.deref().clone());
                    });
                }
            }
        }),
    );

    std::mem::forget(global);
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

    init(&mut display);

    let display = Rc::new(RefCell::new(display));

    let timer = calloop::timer::Timer::new().unwrap();
    let handle = timer.handle();

    event_loop
        .handle()
        .insert_source(timer, {
            let handle = handle.clone();
            move |count, _, _| {
                FOO.with(|data| {
                    let data = data.borrow();

                    for ws in data.iter() {
                        ws.name(format!("{}", count));
                    }
                });

                handle.add_timeout(Duration::from_secs(1), count + 1);
            }
        })
        .unwrap();

    handle.add_timeout(Duration::from_secs(1), 0);

    event_loop
        .run(None, &mut State { display }, |state| {
            state.display.clone().borrow_mut().flush_clients(state);
        })
        .unwrap();
}
