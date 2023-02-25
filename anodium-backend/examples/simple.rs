use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use anodium_backend::udev::{drm_scanner, new_drm_device};
use input::Libinput;
use smithay::{
    backend::{
        allocator::gbm::{self, GbmAllocator, GbmBufferFlags},
        allocator::Format,
        drm::{self, DrmDeviceFd, DrmNode, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        input::{InputEvent, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles2::Gles2Renderer, Bind, Frame, Renderer},
        session::{libseat::LibSeatSession, Session},
        udev::{UdevBackend, UdevEvent},
    },
    output::{Output, PhysicalProperties},
    reexports::{
        calloop::{timer::Timer, EventLoop, LoopHandle},
        drm::control::{connector, crtc, ModeTypeFlags},
    },
    utils::{Rectangle, Transform},
};

struct Surface {
    gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    output: Output,
}

impl Surface {
    fn new(
        crtc: crtc::Handle,
        connector: &connector::Info,
        formats: HashSet<Format>,
        drm: &drm::DrmDevice,
        gbm_allocator: GbmAllocator<DrmDeviceFd>,
    ) -> Self {
        let mode_id = connector
            .modes()
            .iter()
            .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(0);

        let drm_mode = connector.modes()[mode_id];

        let drm_surface = drm
            .create_surface(crtc, drm_mode, &[connector.handle()])
            .unwrap();

        let gbm_surface = GbmBufferedSurface::new(drm_surface, gbm_allocator, formats).unwrap();

        let name = anodium_backend::udev::format_connector_name(connector);

        let (w, h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name,
            PhysicalProperties {
                size: (w as i32, h as i32).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make: "todo".into(),
                model: "todo".into(),
            },
        );
        Self {
            gbm_surface,
            output,
        }
    }
}

struct Device {
    drm: drm::DrmDevice,
    gbm: gbm::GbmDevice<DrmDeviceFd>,
    gbm_allocator: GbmAllocator<DrmDeviceFd>,

    connectors: drm_scanner::ConnectorScanner,
    crtcs: drm_scanner::CrtcsScanner,

    renderer: Gles2Renderer,
    surfaces: HashMap<crtc::Handle, Surface>,
}

struct State {
    handle: LoopHandle<'static, Self>,
    session: LibSeatSession,
    primary_gpu: DrmNode,
    devices: HashMap<DrmNode, Device>,
}

fn next_buffer(surface: &mut Surface, renderer: &mut Gles2Renderer) {
    let (dmabuf, _age) = surface.gbm_surface.next_buffer().unwrap();
    renderer.bind(dmabuf).unwrap();

    let mut frame = renderer
        .render((i32::MAX, i32::MAX).into(), Transform::Normal)
        .unwrap();

    frame
        .clear(
            [1.0, 0.0, 0.0, 1.0],
            &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
        )
        .unwrap();

    frame.finish().unwrap();

    surface.gbm_surface.queue_buffer(None, ()).unwrap();
}

fn on_drm_event(
    state: &mut State,
    node: DrmNode,
    event: drm::DrmEvent,
    _meta: &mut Option<drm::DrmEventMetadata>,
) {
    match event {
        drm::DrmEvent::VBlank(crtc) => {
            if let Some(device) = state.devices.get_mut(&node) {
                if let Some(surface) = device.surfaces.get_mut(&crtc) {
                    surface.gbm_surface.frame_submitted().unwrap();
                    next_buffer(surface, &mut device.renderer);
                }
            }
        }
        drm::DrmEvent::Error(_) => {}
    }
}

fn on_connector_event(state: &mut State, node: DrmNode, event: drm_scanner::ConnectorEvent) {
    let device = if let Some(device) = state.devices.get_mut(&node) {
        device
    } else {
        return;
    };

    match event {
        drm_scanner::ConnectorEvent::Connected(connector) => {
            if let Some(crtc) = device.crtcs.for_connector(&device.drm, &connector) {
                let mut surface = Surface::new(
                    crtc,
                    &connector,
                    device
                        .renderer
                        .egl_context()
                        .dmabuf_render_formats()
                        .clone(),
                    &device.drm,
                    device.gbm_allocator.clone(),
                );

                next_buffer(&mut surface, &mut device.renderer);

                device.surfaces.insert(crtc, surface);
            }
        }
        drm_scanner::ConnectorEvent::Disconnected(connector) => {
            if let Some(crtc) = device.crtcs.remove_connector(&connector.handle()) {
                device.surfaces.remove(&crtc);
            }
        }
    }
}

fn on_device_added(state: &mut State, node: DrmNode, path: PathBuf) {
    let (drm_device, drm_notifier) = new_drm_device(&mut state.session, &path);

    let mut crtcs_scanner = drm_scanner::CrtcsScanner::new();
    crtcs_scanner.scan_crtcs(&drm_device);

    let gbm_device = gbm::GbmDevice::new(drm_device.device_fd().clone()).unwrap();
    let gbm_allocator = GbmAllocator::new(
        gbm_device.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );

    let display = EGLDisplay::new(gbm_device.clone()).unwrap();
    let context = EGLContext::new(&display).unwrap();
    let renderer = unsafe { Gles2Renderer::new(context) }.unwrap();

    state
        .handle
        .insert_source(drm_notifier, move |event, meta, state| {
            on_drm_event(state, node, event, meta)
        })
        .unwrap();

    state.devices.insert(
        node,
        Device {
            drm: drm_device,
            gbm: gbm_device,
            gbm_allocator,

            connectors: Default::default(),
            crtcs: crtcs_scanner,

            renderer,
            surfaces: Default::default(),
        },
    );

    on_device_changed(state, node);
}

fn on_device_changed(state: &mut State, node: DrmNode) {
    if let Some(scan) = state
        .devices
        .get_mut(&node)
        .map(|device| device.connectors.scan_connectors(&device.drm))
    {
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

    let (primary_gpu, _) = anodium_backend::udev::primary_gpu(&session.seat());

    let mut state = State {
        handle: event_loop.handle(),
        session,
        devices: Default::default(),
        primary_gpu,
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
                    std::process::exit(0);
                }
            }
        })
        .unwrap();
}