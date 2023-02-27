use std::{
    collections::{HashMap, HashSet},
    os::unix::prelude::FromRawFd,
    path::PathBuf,
    time::Duration,
};

use anodium_backend::udev::{
    drm_mode_to_wl_mode,
    drm_scanner::{self, DrmScanEvent},
    edid::EdidInfo,
};

use smithay::{
    backend::{
        allocator::Format,
        allocator::{
            dmabuf::{Dmabuf, DmabufAllocator},
            gbm::{self, GbmAllocator, GbmBufferFlags},
        },
        drm::{self, DrmDeviceFd, DrmNode, GbmBufferedSurface},
        egl::{EGLDevice, EGLDisplay},
        input::{InputEvent, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::DamageTrackedRenderer,
            element::memory::MemoryRenderBufferRenderElement,
            gles2::Gles2Renderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
            Bind, ImportMem, Renderer,
        },
        session::{libseat::LibSeatSession, Session},
        udev::{UdevBackend, UdevEvent},
    },
    output::{Output, PhysicalProperties},
    reexports::{
        calloop::{timer::Timer, EventLoop, LoopHandle},
        drm::control::{connector, crtc, ModeTypeFlags},
        input::Libinput,
        nix::fcntl::OFlag,
    },
    utils::{DeviceFd, Transform},
};

struct Surface {
    gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    output: Output,
    damage_tracked_renderer: DamageTrackedRenderer,
}

impl Surface {
    fn new(
        crtc: crtc::Handle,
        connector: &connector::Info,
        formats: HashSet<Format>,
        drm: &drm::DrmDevice,
        gbm: gbm::GbmDevice<DrmDeviceFd>,
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

        let gbm_surface = GbmBufferedSurface::new(
            drm_surface,
            GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
            formats,
        )
        .unwrap();

        let name = anodium_backend::udev::format_connector_name(connector);

        let (make, model) = EdidInfo::for_connector(drm, connector.handle())
            .map(|info| (info.manufacturer, info.model))
            .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));

        let (w, h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name,
            PhysicalProperties {
                size: (w as i32, h as i32).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make,
                model,
            },
        );

        let output_mode = drm_mode_to_wl_mode(drm_mode);
        output.set_preferred(output_mode);
        output.change_current_state(
            Some(output_mode),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Integer(1)),
            None,
        );

        let damage_tracked_renderer = DamageTrackedRenderer::from_output(&output);

        Self {
            gbm_surface,
            output,
            damage_tracked_renderer,
        }
    }
}

struct Device {
    drm: drm::DrmDevice,
    gbm: gbm::GbmDevice<DrmDeviceFd>,
    gbm_allocator: DmabufAllocator<GbmAllocator<DrmDeviceFd>>,

    drm_scanner: drm_scanner::DrmScanner,

    surfaces: HashMap<crtc::Handle, Surface>,
    render_node: DrmNode,
}

struct State {
    handle: LoopHandle<'static, Self>,
    session: LibSeatSession,
    primary_gpu: DrmNode,
    gpu_manager: GpuManager<GbmGlesBackend<Gles2Renderer>>,
    devices: HashMap<DrmNode, Device>,
}

fn next_buffer<R>(surface: &mut Surface, renderer: &mut R)
where
    R: Renderer + ImportMem + Bind<Dmabuf>,
    R::TextureId: 'static,
{
    let (dmabuf, age) = surface.gbm_surface.next_buffer().unwrap();
    renderer.bind(dmabuf).unwrap();

    surface
        .damage_tracked_renderer
        .render_output::<MemoryRenderBufferRenderElement<R>, _>(
            renderer,
            age as usize,
            &[],
            [1.0, 0.0, 0.0, 1.0],
        )
        .unwrap();

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
                    let mut renderer = if state.primary_gpu == device.render_node {
                        state
                            .gpu_manager
                            .single_renderer(&device.render_node)
                            .unwrap()
                    } else {
                        state
                            .gpu_manager
                            .renderer(
                                &state.primary_gpu,
                                &device.render_node,
                                &mut device.gbm_allocator,
                                surface.gbm_surface.format(),
                            )
                            .unwrap()
                    };

                    surface.gbm_surface.frame_submitted().unwrap();
                    next_buffer(surface, &mut renderer);
                }
            }
        }
        drm::DrmEvent::Error(_) => {}
    }
}

fn on_connector_event(state: &mut State, node: DrmNode, event: drm_scanner::DrmScanEvent) {
    let device = if let Some(device) = state.devices.get_mut(&node) {
        device
    } else {
        return;
    };

    match event {
        DrmScanEvent::Connected {
            connector,
            crtc: Some(crtc),
        } => {
            let mut renderer = state
                .gpu_manager
                .single_renderer(&device.render_node)
                .unwrap();

            let mut surface = Surface::new(
                crtc,
                &connector,
                renderer
                    .as_mut()
                    .egl_context()
                    .dmabuf_render_formats()
                    .clone(),
                &device.drm,
                device.gbm.clone(),
            );

            next_buffer(&mut surface, renderer.as_mut());

            device.surfaces.insert(crtc, surface);
        }
        DrmScanEvent::Disconnected {
            crtc: Some(crtc), ..
        } => {
            device.surfaces.remove(&crtc);
        }
        _ => {}
    }
}

fn on_device_added(state: &mut State, node: DrmNode, path: PathBuf) {
    let fd = state
        .session
        .open(
            &path,
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
        )
        .unwrap();

    let fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd) });

    let (drm, drm_notifier) = drm::DrmDevice::new(fd, false).unwrap();

    let gbm = gbm::GbmDevice::new(drm.device_fd().clone()).unwrap();
    let gbm_allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING);

    // Make sure display is dropped before we call add_node
    let render_node = match EGLDevice::device_for_display(&EGLDisplay::new(gbm.clone()).unwrap())
        .ok()
        .and_then(|x| x.try_get_render_node().ok().flatten())
    {
        Some(node) => node,
        None => node,
    };

    state
        .gpu_manager
        .as_mut()
        .add_node(render_node, gbm.clone())
        .unwrap();

    state
        .handle
        .insert_source(drm_notifier, move |event, meta, state| {
            on_drm_event(state, node, event, meta)
        })
        .unwrap();

    state.devices.insert(
        node,
        Device {
            drm,
            gbm,
            gbm_allocator: DmabufAllocator(gbm_allocator),

            drm_scanner: Default::default(),

            surfaces: Default::default(),
            render_node,
        },
    );

    on_device_changed(state, node);
}

fn on_device_changed(state: &mut State, node: DrmNode) {
    if let Some(device) = state.devices.get_mut(&node) {
        for event in device.drm_scanner.scan_connectors(&device.drm) {
            on_connector_event(state, node, event);
        }
    }
}

fn on_device_removed(state: &mut State, node: DrmNode) {
    if let Some(device) = state.devices.get_mut(&node) {
        state.gpu_manager.as_mut().remove_node(&device.render_node);
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
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                on_device_removed(state, node);
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
        gpu_manager: GpuManager::new(Default::default()).unwrap(),
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
