#![allow(irrefutable_let_patterns)]

use anodium_backend::BackendState;
use anodium_framework::pointer_icon::PointerIcon;

use clap::StructOpt;
use on_commit::OnCommitDispatcher;
use slog::Drain;
use smithay::{
    desktop::{self, PopupManager},
    reexports::{
        calloop::{
            generic::Generic, EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction,
        },
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            Display, DisplayHandle, Resource,
        },
    },
    wayland::{
        compositor::CompositorState,
        data_device::{self, DataDeviceState},
        dmabuf::DmabufState,
        output::OutputManagerState,
        seat::{Seat, SeatState},
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use std::{ffi::OsString, sync::Arc, time::Instant};

#[cfg(feature = "xwayland")]
use smithay::xwayland::XWayland;

mod cli;
mod data;
mod grabs;
mod handlers;
mod on_commit;

struct CalloopData {
    state: State,
    display: Display<State>,
}

pub struct State {
    space: desktop::Space,
    popups: PopupManager,

    display: DisplayHandle,

    start_time: Instant,
    loop_signal: LoopSignal,
    _loop_handle: LoopHandle<'static, CalloopData>,

    seat: Seat<Self>,

    on_commit_dispatcher: OnCommitDispatcher,

    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    _output_manager_state: OutputManagerState,
    seat_state: SeatState<Self>,
    data_device_state: DataDeviceState,
    dmabuf_state: DmabufState,

    pointer_icon: PointerIcon,

    backend: BackendState,

    socket_name: OsString,

    #[cfg(feature = "xwayland")]
    xwayland: XWayland,
}

/// init the xwayland connection
#[cfg(feature = "xwayland")]
fn init_xwayland_connection(
    handle: &LoopHandle<'static, CalloopData>,
    display: &DisplayHandle,
) -> XWayland {
    use smithay::xwayland::XWaylandEvent;

    let (xwayland, channel) = XWayland::new(slog_scope::logger(), display);

    handle
        .insert_source(channel, {
            let handle = handle.clone();
            move |event, _, state| match event {
                XWaylandEvent::Ready {
                    connection,
                    client,
                    client_fd,
                    display,
                } => {
                    // state
                    // .shell_manager
                    // .xwayland_ready(&handle, connection, client)
                }
                XWaylandEvent::Exited => {}
            }
        })
        .unwrap();

    xwayland
}

struct ClientState;
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

fn init_wayland_listener<D>(
    display: &mut Display<D>,
    event_loop: &mut EventLoop<CalloopData>,
    log: slog::Logger,
) -> OsString {
    // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
    let listening_socket = ListeningSocketSource::new_auto(log).unwrap();

    // Get the name of the listening socket.
    // Clients will connect to this socket.
    let socket_name = listening_socket.socket_name().to_os_string();

    let handle = event_loop.handle();

    event_loop
        .handle()
        .insert_source(listening_socket, move |client_stream, _, state| {
            // Inside the callback, you should insert the client into the display.
            //
            // You may also associate some data with the client when inserting the client.
            state
                .display
                .handle()
                .insert_client(client_stream, Arc::new(ClientState))
                .unwrap();
        })
        .expect("Failed to init the wayland event source.");

    // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
    handle
        .insert_source(
            Generic::new(display.backend().poll_fd(), Interest::READ, Mode::Level),
            |_, _, state| {
                state.display.dispatch_clients(&mut state.state).unwrap();
                Ok(PostAction::Continue)
            },
        )
        .unwrap();

    socket_name
}

fn init_log() -> slog::Logger {
    let terminal_drain = slog_envlogger::LogBuilder::new(
        slog_term::CompactFormat::new(slog_term::TermDecorator::new().stderr().build())
            .build()
            .fuse(),
    )
    .filter(Some("anodium"), slog::FilterLevel::Trace)
    .filter(Some("smithay"), slog::FilterLevel::Warning)
    .build()
    .fuse();

    let terminal_drain = slog_async::Async::default(terminal_drain).fuse();

    let log = slog::Logger::root(terminal_drain.fuse(), slog::o!());

    slog_stdlog::init().expect("Could not setup log backend");

    log
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log = init_log();
    let _guard = slog_scope::set_global_logger(log);

    let opt = cli::AnodiumCliOptions::parse();

    let mut event_loop = EventLoop::<CalloopData>::try_new()?;
    let mut display = Display::new()?;

    let socket_name = init_wayland_listener(&mut display, &mut event_loop, slog_scope::logger());

    let pointer_icon = PointerIcon::new();

    let dh = display.handle();
    let compositor_state = CompositorState::new::<State, _>(&dh, slog_scope::logger());
    let xdg_shell_state = XdgShellState::new::<State, _>(&dh, slog_scope::logger());
    let shm_state = ShmState::new::<State, _>(&dh, vec![], slog_scope::logger());
    let output_manager_state = OutputManagerState::new_with_xdg_output::<State>(&dh);
    let seat_state = SeatState::<State>::new();
    let data_device_state = DataDeviceState::new::<State, _>(&dh, slog_scope::logger());

    let dmabuf_state = DmabufState::new();

    let mut seat = Seat::<State>::new(&display.handle(), "seat0", slog_scope::logger());

    seat.add_pointer({
        let pointer_icon = pointer_icon.clone();
        move |cursor| pointer_icon.on_new_cursor(cursor)
    });

    seat.add_keyboard(Default::default(), 200, 25, move |seat, focus| {
        let focus = focus.and_then(|s| dh.get_client(s.id()).ok());
        data_device::set_data_device_focus(&dh, seat, focus);
    })?;

    #[cfg(feature = "xwayland")]
    let xwayland = init_xwayland_connection(&event_loop.handle(), &display.handle());

    let state = State {
        space: desktop::Space::new(slog_scope::logger()),
        popups: PopupManager::new(slog_scope::logger()),
        display: display.handle(),

        start_time: Instant::now(),
        loop_signal: event_loop.get_signal(),
        _loop_handle: event_loop.handle(),

        seat,

        on_commit_dispatcher: Default::default(),

        compositor_state,
        xdg_shell_state,
        shm_state,
        _output_manager_state: output_manager_state,
        seat_state,
        data_device_state,
        dmabuf_state,

        pointer_icon,
        backend: BackendState::default(),

        socket_name,
        #[cfg(feature = "xwayland")]
        xwayland,
    };

    let mut data = CalloopData { state, display };

    anodium_backend::init(
        &mut event_loop,
        &data.display.handle(),
        &mut data,
        opt.backend,
    );

    event_loop.run(None, &mut data, |data| {
        data.state.space.refresh(&data.display.handle());
        data.state.popups.cleanup();
        data.display.flush_clients().unwrap();
    })?;

    Ok(())
}
