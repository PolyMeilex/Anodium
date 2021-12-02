use glib::MainContext;
use wayland_client::{DispatchData, Display, GlobalManager};

use super::{init_global, AnodiumOutput};

pub fn init<F, D>(mut data: D, cb: F) -> glib::SourceId
where
    F: Fn(AnodiumOutput, DispatchData) + 'static,
    D: 'static,
{
    // Connect to the server
    let display = Display::connect_to_env().unwrap();

    let mut event_queue = display.create_event_queue();

    let attached_display = (*display).clone().attach(event_queue.token());

    // We use the GlobalManager convenience provided by the crate, it covers
    // most classic use cases and avoids us the trouble to manually implement
    // the registry
    let globals = GlobalManager::new(&attached_display);

    // A roundtrip synchronization to make sure the server received our registry
    // creation and sent us the global list
    event_queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();

    init_global(&globals, cb);

    event_queue.sync_roundtrip(&mut (), |_, _, _| {}).unwrap();

    let fd = display.get_connection_fd();

    let c = MainContext::default();
    let _guard = c.acquire().unwrap();
    glib::source::unix_fd_add_local(fd, glib::IOCondition::IN, move |_, _| {
        let res = event_queue.dispatch(&mut data, |_, _, _| {});
        glib::Continue(res.is_ok())
    })
}
