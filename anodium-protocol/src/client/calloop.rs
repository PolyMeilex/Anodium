use calloop::{LoopHandle, RegistrationToken};

use std::{cell::RefCell, error::Error, rc::Rc};

use wayland_client::{DispatchData, Display, EventQueue, GlobalManager};

use super::{init_global, AnodiumOutput};

pub fn init<D, F>(
    handle: LoopHandle<'static, D>,
    cb: F,
) -> Result<RegistrationToken, Box<dyn Error + Send + Sync>>
where
    D: 'static,
    F: Fn(AnodiumOutput, DispatchData) + 'static,
{
    // Connect to the server
    let display = Display::connect_to_env()?;

    let mut event_queue = display.create_event_queue();

    let attached_display = (*display).clone().attach(event_queue.token());

    // We use the GlobalManager convenience provided by the crate, it covers
    // most classic use cases and avoids us the trouble to manually implement
    // the registry
    let globals = GlobalManager::new(&attached_display);

    // A roundtrip synchronization to make sure the server received our registry
    // creation and sent us the global list
    event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

    init_global(&globals, cb);

    event_queue.sync_roundtrip(&mut (), |_, _, _| {})?;

    let event_queue = Rc::new(RefCell::new(event_queue));
    insert_wayland_source(handle, &display, event_queue)
}

fn insert_wayland_source<D: 'static>(
    handle: LoopHandle<'static, D>,
    display: &Display,
    event_queue: Rc<RefCell<EventQueue>>,
) -> Result<RegistrationToken, Box<dyn Error + Send + Sync>> {
    let token = handle.insert_source(
        calloop::generic::Generic::from_fd(
            display.get_connection_fd(),
            calloop::Interest::READ,
            calloop::Mode::Level,
        ),
        move |_, _, state| {
            event_queue
                .clone()
                .borrow_mut()
                .dispatch(state, |_, _, _| {})
                .unwrap();

            Ok(calloop::PostAction::Continue)
        },
    )?;

    Ok(token)
}
