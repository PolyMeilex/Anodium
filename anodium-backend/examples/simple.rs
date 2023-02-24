use std::{collections::HashMap, path::PathBuf, time::Duration};

use anodium_backend::udev::{drm_device, drm_scanner, gbm_device};
use input::Libinput;
use smithay::{
    backend::{
        allocator::gbm::{self, GbmAllocator, GbmBufferFlags},
        drm::{self, DrmDeviceFd, DrmNode, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        input::{InputEvent, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles2::Gles2Renderer, Bind, Frame, Renderer},
        session::{libseat::LibSeatSession, Session},
        udev::{UdevBackend, UdevEvent},
    },
    reexports::{
        calloop::{timer::Timer, EventLoop, LoopHandle},
        drm::control::{crtc, ModeTypeFlags},
    },
    utils::Rectangle,
};

struct Device {
    drm_device: drm_device::DrmDevice,
    gbm_device: gbm::GbmDevice<DrmDeviceFd>,
    gbm_allocator: GbmAllocator<DrmDeviceFd>,

    connector_scanner: drm_scanner::ConnectorScanner,
    crtcs_scanner: drm_scanner::CrtcsScanner,

    renderer: Gles2Renderer,
    surfaces: HashMap<crtc::Handle, GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>>,
}

struct State {
    handle: LoopHandle<'static, Self>,
    session: LibSeatSession,
    devices: HashMap<DrmNode, Device>,
}

fn on_drm_event(
    state: &mut State,
    device: DrmNode,
    event: drm::DrmEvent,
    _meta: &mut Option<drm::DrmEventMetadata>,
) {
    match event {
        drm::DrmEvent::VBlank(crtc) => {
            if let Some(device) = state.devices.get_mut(&device) {
                if let Some(surface) = device.surfaces.get_mut(&crtc) {
                    surface.frame_submitted().unwrap();

                    let (dmabuf, age) = surface.next_buffer().unwrap();
                    device.renderer.bind(dmabuf).unwrap();

                    let mut frame = device
                        .renderer
                        .render(
                            (i32::MAX, i32::MAX).into(),
                            smithay::utils::Transform::Normal,
                        )
                        .unwrap();

                    frame
                        .clear(
                            [1.0, 0.0, 0.0, 1.0],
                            &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
                        )
                        .unwrap();

                    frame.finish().unwrap();

                    surface.queue_buffer(None, ()).unwrap();
                }
            }
        }
        drm::DrmEvent::Error(_) => {}
    }
}

fn on_connector_event(
    state: &mut State,
    node: DrmNode,
    event: drm_scanner::ConnectorEvent,
) {
    let device = if let Some(device) = state.devices.get_mut(&node) {
        device
    } else {
        return;
    };

    match event {
        drm_scanner::ConnectorEvent::Connected(connector) => {
            if let Some(crtc) = device
                .crtcs_scanner
                .for_connector(&device.drm_device.borrow(), connector.clone())
            {
                let mode_id = connector
                    .modes()
                    .iter()
                    .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                    .unwrap_or(0);

                let drm_mode = connector.modes()[mode_id];

                let mut gbm_surface = gbm_device::create_surface(
                    &device.drm_device,
                    device.gbm_allocator.clone(),
                    crtc,
                    connector.handle(),
                    drm_mode,
                    device
                        .renderer
                        .egl_context()
                        .dmabuf_render_formats()
                        .clone(),
                );

                gbm_surface.next_buffer().unwrap();
                gbm_surface.queue_buffer(None, ()).unwrap();

                device.surfaces.insert(crtc, gbm_surface);
            }
        }
        drm_scanner::ConnectorEvent::Disconnected(connector) => {
            if let Some(crtc) = device.crtcs_scanner.remove_connector(&connector) {
                device.surfaces.remove(&crtc);
            }
        }
    }
}

fn on_device_added(state: &mut State, node: DrmNode, path: PathBuf) {
    let drm_device = drm_device::DrmDevice::new(&mut state.session, node, &path);
    let crtcs_scanner = drm_scanner::CrtcsScanner::default();

    let gbm_device = gbm::GbmDevice::new(drm_device.fd()).unwrap();
    let gbm_allocator = GbmAllocator::new(
        gbm_device.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );

    let display = EGLDisplay::new(gbm_device.clone()).unwrap();
    let context = EGLContext::new(&display).unwrap();
    let renderer = unsafe { Gles2Renderer::new(context) }.unwrap();

    state
        .handle
        .insert_source(drm_device.clone(), move |event, meta, state| {
            on_drm_event(state, node, event, meta)
        })
        .unwrap();

    state.devices.insert(
        drm_device.node(),
        Device {
            drm_device,
            gbm_device,
            gbm_allocator,

            connector_scanner: Default::default(),
            crtcs_scanner,

            renderer,
            surfaces: Default::default(),
        },
    );

    on_device_changed(state, node);
}

fn on_device_changed(state: &mut State, node: DrmNode) {
    if let Some(scan) = state.devices.get_mut(&node).map(|device| {
        device
            .connector_scanner
            .scan_connectors(&device.drm_device.borrow())
    }) {
        for event in scan {
            on_connector_event(state, node, event);
        }
    }
}

fn on_udev_event(state: &mut State, event: UdevEvent) {
    match event {
        UdevEvent::Added { device_id, path } => {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                on_device_added(state, node, path);
            }
        }
        UdevEvent::Changed { device_id } => {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                on_device_changed(state, node);
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
        .insert_source(Timer::from_duration(Duration::from_secs(60)), |_, _, _| {
            panic!("Aborted");
        })
        .unwrap();

    event_loop.run(None, &mut state, |_data| {})?;

    Ok(())
}

fn init_udev(state: &mut State) {
    let backend = UdevBackend::new(state.session.seat()).unwrap();
    for (device_id, path) in backend.device_list() {
        on_udev_event(
            state,
            UdevEvent::Added {
                device_id,
                path: path.to_owned(),
            },
        );
    }

    state
        .handle
        .insert_source(backend, |event, _, state| on_udev_event(state, event))
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
