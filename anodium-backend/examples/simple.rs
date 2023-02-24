use std::{collections::HashMap, path::PathBuf, time::Duration};

use anodium_backend::udev::{drm_scanner, drm_device, gbm_device};
use input::Libinput;
use smithay::{
    backend::{
        drm::{self, DrmNode},
        input::{InputEvent, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session},
        udev::{UdevBackend, UdevEvent},
    },
    reexports::calloop::{timer::Timer, EventLoop, LoopHandle},
};

struct Device {
    drm_scanner: drm_scanner::ConnectorScanner,
    drm_device: drm_device::DrmDevice,
    gbm_device: gbm_device::GbmDevice,
}

struct State {
    handle: LoopHandle<'static, Self>,
    session: LibSeatSession,
    devices: HashMap<DrmNode, Device>,
}

fn drm_event_handler(
    state: &mut State,
    device: DrmNode,
    event: drm::DrmEvent,
    _meta: &mut Option<drm::DrmEventMetadata>,
) {
    match event {
        drm::DrmEvent::VBlank(crtc) => {
            if let Some(device) = state.devices.get_mut(&device) {
                device.gbm_device.vblank(crtc);
            }
        }
        drm::DrmEvent::Error(_) => {}
    }
}

fn drm_connector_event_handler(
    _state: &mut State,
    _device: DrmNode,
    event: drm_scanner::ConnectorEvent,
) {
    dbg!(&event);
    match event {
        drm_scanner::ConnectorEvent::Connected(_connector) => {}
        drm_scanner::ConnectorEvent::Disconnected(_connector) => {}
    }
}

fn udev_added_event_handler(state: &mut State, node: DrmNode, path: PathBuf) {
    let drm_device = drm_device::DrmDevice::new(&mut state.session, node, &path);
    let gbm_device = gbm_device::GbmDevice::new(&drm_device);

    state
        .handle
        .insert_source(drm_device.clone(), move |event, meta, state| {
            drm_event_handler(state, node, event, meta)
        })
        .unwrap();

    state.devices.insert(
        drm_device.node(),
        Device {
            drm_scanner: Default::default(),
            drm_device,
            gbm_device,
        },
    );

    udev_changed_event_handler(state, node);
}

fn udev_changed_event_handler(state: &mut State, node: DrmNode) {
    if let Some(scan) = state.devices.get_mut(&node).map(|device| {
        device
            .drm_scanner
            .scan_connectors(&device.drm_device.borrow())
    }) {
        for event in scan {
            drm_connector_event_handler(state, node, event);
        }
    }
}

fn udev_event_handler(state: &mut State, event: UdevEvent) {
    match event {
        UdevEvent::Added { device_id, path } => {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                udev_added_event_handler(state, node, path);
            }
        }
        UdevEvent::Changed { device_id } => {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                udev_changed_event_handler(state, node);
            }
        }
        UdevEvent::Removed { device_id } => {
            if let Ok(_node) = DrmNode::from_dev_id(device_id) {
                //
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<State>::try_new()?;

    let (session, notify) = LibSeatSession::new().unwrap();

    event_loop
        .handle()
        .insert_source(notify, |_, _, _| {})
        .unwrap();

    let mut state = State {
        handle: event_loop.handle(),
        session,
        devices: Default::default(),
    };

    init_input(&state);
    init_udev(&mut state);

    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(20)), |_, _, _| {
            panic!("Aborted");
        })
        .unwrap();

    event_loop.run(None, &mut state, |_data| {})?;

    Ok(())
}

fn init_udev(state: &mut State) {
    let backend = UdevBackend::new(state.session.seat()).unwrap();
    for (device_id, path) in backend.device_list() {
        udev_event_handler(
            state,
            UdevEvent::Added {
                device_id,
                path: path.to_owned(),
            },
        );
    }

    state
        .handle
        .insert_source(backend, |event, _, state| udev_event_handler(state, event))
        .unwrap();
}

fn init_input(state: &State) {
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        state.session.clone().into(),
    );
    libinput_context
        .udev_assign_seat(&state.session.seat())
        .unwrap();

    let libinput_backend = LibinputInputBackend::new(libinput_context);

    state
        .handle
        .insert_source(libinput_backend, move |event, _, _| {
            if let InputEvent::Keyboard { event } = event {
                if event.key_code() == 59 {
                    panic!("exit");
                }
            }
        })
        .unwrap();
}
