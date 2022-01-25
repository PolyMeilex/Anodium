use std::{
    cell::RefCell,
    collections::hash_map::{Entry, HashMap},
    io::Error as IoError,
    os::unix::io::{AsRawFd, RawFd},
    path::{Path, PathBuf},
    rc::Rc,
};

use image::ImageBuffer;

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        drm::{DrmDevice, DrmError, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles2::{Gles2Renderer, Gles2Texture},
            Bind, Frame, ImportDma, ImportEgl, Renderer,
        },
        session::{
            auto::{AutoSession, AutoSessionNotifier},
            Session, Signal as SessionSignal,
        },
        udev::{primary_gpu, UdevBackend, UdevEvent},
        SwapBuffersError,
    },
    reexports::{
        calloop::{
            channel,
            timer::{Timer, TimerHandle},
            Dispatcher, EventLoop, LoopHandle,
        },
        drm::{
            self,
            control::{
                connector::{self, Info as ConnectorInfo, State as ConnectorState},
                crtc,
                encoder::Info as EncoderInfo,
                Device as ControlDevice, Mode as DrmMode,
            },
        },
        gbm::Device as GbmDevice,
        input::Libinput,
        nix::{fcntl::OFlag, sys::stat::dev_t},
        wayland_server::{protocol::wl_output, Display},
    },
    utils::{
        signaling::{Linkable, SignalToken, Signaler},
        Rectangle, Transform,
    },
    wayland::{
        dmabuf::init_dmabuf_global,
        output::{Mode, PhysicalProperties},
    },
};

use super::{BackendHandler, BackendRequest};
use crate::{
    framework,
    output_manager::{Output, OutputDescriptor},
    render::renderer::import_bitmap,
};

struct Inner {
    display: Rc<RefCell<Display>>,
    primary_gpu: Option<PathBuf>,
    session: AutoSession,
    pointer_image: framework::cursor::Cursor,
    udev_devices: HashMap<dev_t, UdevDeviceData>,

    outputs: Vec<Output>,
}

type InnerRc = Rc<RefCell<Inner>>;

#[derive(Clone)]
pub struct SessionFd(RawFd);
impl AsRawFd for SessionFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

#[derive(Debug, PartialEq)]
struct UdevOutputId {
    device_id: dev_t,
    crtc: crtc::Handle,
}

type RenderTimerHandle = TimerHandle<(u64, crtc::Handle)>;

pub fn run_udev<D>(
    event_loop: &mut EventLoop<'static, D>,
    handler: &mut D,
    mut session: AutoSession,
    notifier: AutoSessionNotifier,
    rx: channel::Channel<BackendRequest>,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let log = slog_scope::logger();

    let session_signal = notifier.signaler();

    let display = handler.wl_display();

    /*
     * Initialize the compositor
     */

    let primary_gpu = primary_gpu(&session.seat()).unwrap_or_default();
    info!("Primary GPU: {:?}", primary_gpu);

    let inner = Rc::new(RefCell::new(Inner {
        display: display.clone(),
        primary_gpu,
        session: session.clone(),
        pointer_image: framework::cursor::Cursor::load(&log),
        udev_devices: Default::default(),
        outputs: Default::default(),
    }));

    /*
     * Initialize the udev backend
     */
    let udev_backend = UdevBackend::new(session.seat(), log.clone()).map_err(|_| ())?;

    /*
     * Initialize libinput backend
     */
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<AutoSession>>(session.clone().into());
    libinput_context.udev_assign_seat(&session.seat()).unwrap();
    let mut libinput_backend = LibinputInputBackend::new(libinput_context, log.clone());
    libinput_backend.link(session_signal.clone());

    /*
     * Bind all our objects that get driven by the event loop
     */
    let _libinput_event_source = event_loop
        .handle()
        .insert_source(libinput_backend, move |mut event, _, handler| {
            match &mut event {
                InputEvent::DeviceAdded { device } => {
                    device.config_tap_set_enabled(true).ok();
                }
                InputEvent::DeviceRemoved { .. } => {}
                _ => {}
            }

            handler.process_input_event(event, None);
        })
        .unwrap();
    let _session_event_source = event_loop
        .handle()
        .insert_source(notifier, |(), &mut (), _anvil_state| {})
        .unwrap();

    for (dev, path) in udev_backend.device_list() {
        device_added(
            handler,
            event_loop.handle(),
            inner.clone(),
            dev,
            path.into(),
            &session_signal,
        )
    }

    // init dmabuf support with format list from all gpus
    // TODO: We need to update this list, when the set of gpus changes
    // TODO2: This does not necessarily depend on egl, but mesa makes no use of it without wl_drm right now
    {
        let mut formats = Vec::new();
        for backend_data in inner.borrow().udev_devices.values() {
            formats.extend(backend_data.renderer.borrow().dmabuf_formats().cloned());
        }

        init_dmabuf_global(
            &mut *display.borrow_mut(),
            formats,
            {
                let inner = inner.clone();

                move |buffer, _| {
                    let inner = inner.borrow();

                    for backend_data in inner.udev_devices.values() {
                        if backend_data
                            .renderer
                            .borrow_mut()
                            .import_dmabuf(buffer)
                            .is_ok()
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            slog_scope::logger(),
        );
    }

    event_loop
        .handle()
        .insert_source(rx, move |event, _, _| match event {
            channel::Event::Msg(event) => match event {
                BackendRequest::ChangeVT(id) => {
                    session.change_vt(id).ok();
                }
            },
            channel::Event::Closed => {}
        })
        .unwrap();

    let handle = event_loop.handle();
    let _udev_event_source = event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, handler| match event {
            UdevEvent::Added { device_id, path } => device_added(
                handler,
                handle.clone(),
                inner.clone(),
                device_id,
                path,
                &session_signal,
            ),
            UdevEvent::Changed { device_id } => {
                error!("Udev device ({:?}) changed: unimplemented", device_id);
            }
            UdevEvent::Removed { device_id } => {
                error!("Udev device ({:?}) removed: unimplemented", device_id);
            }
        })
        .map_err(|e| -> IoError { e.into() })
        .unwrap();

    /*
     * Start XWayland and Wayland Socket
     */
    handler.start_compositor();

    // Cleanup stuff

    // event_loop.handle().remove(session_event_source);
    // event_loop.handle().remove(libinput_event_source);
    // event_loop.handle().remove(udev_event_source);

    Ok(())
}

pub type RenderSurface = GbmBufferedSurface<Rc<RefCell<GbmDevice<SessionFd>>>, SessionFd>;

struct OutputSurfaceData {
    output_descriptor: OutputDescriptor,

    wl_mode: Mode,
    wl_modes: Vec<Mode>,

    drm_modes: Vec<DrmMode>,

    surface: RenderSurface,
    _render_timer: RenderTimerHandle,
    _connector_info: connector::Info,
    crtc: crtc::Handle,
}

pub struct UdevDeviceData {
    _restart_token: SignalToken,
    surfaces: Rc<RefCell<HashMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>>>,
    pointer_images: Vec<(xcursor::parser::Image, Gles2Texture)>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    // gbm: GbmDevice<SessionFd>,
    // registration_token: RegistrationToken,
    // event_dispatcher: Dispatcher<'static, DrmDevice<SessionFd>, BackendState>,
    dev_id: u64,
}

struct ConnectorScanResult {
    backends: HashMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>,
    backends_order: Vec<crtc::Handle>,
}

fn scan_connectors<D>(
    handler: &mut D,
    inner: InnerRc,
    handle: LoopHandle<'static, D>,
    drm: &mut DrmDevice<SessionFd>,
    gbm: &Rc<RefCell<GbmDevice<SessionFd>>>,
    renderer: &mut Gles2Renderer,
    signaler: &Signaler<SessionSignal>,
) -> ConnectorScanResult
where
    D: BackendHandler + 'static,
{
    // Get a set of all modesetting resource handles (excluding planes):
    let res_handles = drm.resource_handles().unwrap();

    // Use first connected connector
    let connector_infos: Vec<ConnectorInfo> = res_handles
        .connectors()
        .iter()
        .map(|conn| drm.get_connector(*conn).unwrap())
        .filter(|conn| conn.state() == ConnectorState::Connected)
        .inspect(|conn| info!("Connected: {:?}", conn.interface()))
        .collect();

    let mut backends = HashMap::new();
    let mut backends_order = Vec::new();

    // very naive way of finding good crtc/encoder/connector combinations. This problem is np-complete
    for connector_info in connector_infos {
        let encoder_infos = connector_info
            .encoders()
            .iter()
            .filter_map(|e| *e)
            .flat_map(|encoder_handle| drm.get_encoder(encoder_handle))
            .collect::<Vec<EncoderInfo>>();
        'outer: for encoder_info in encoder_infos {
            for crtc in res_handles.filter_crtcs(encoder_info.possible_crtcs()) {
                if let Entry::Vacant(entry) = backends.entry(crtc) {
                    info!(
                        "Trying to setup connector {:?}-{} with crtc {:?}",
                        connector_info.interface(),
                        connector_info.interface_id(),
                        crtc,
                    );

                    let output_name = {
                        let other_short_name;
                        let interface_short_name = match connector_info.interface() {
                            drm::control::connector::Interface::DVII => "DVI-I",
                            drm::control::connector::Interface::DVID => "DVI-D",
                            drm::control::connector::Interface::DVIA => "DVI-A",
                            drm::control::connector::Interface::SVideo => "S-VIDEO",
                            drm::control::connector::Interface::DisplayPort => "DP",
                            drm::control::connector::Interface::HDMIA => "HDMI-A",
                            drm::control::connector::Interface::HDMIB => "HDMI-B",
                            drm::control::connector::Interface::EmbeddedDisplayPort => "eDP",
                            other => {
                                other_short_name = format!("{:?}", other);
                                &other_short_name
                            }
                        };
                        format!("{}-{}", interface_short_name, connector_info.interface_id())
                    };

                    let drm_modes = connector_info.modes();

                    let (phys_w, phys_h) = connector_info.size().unwrap_or((0, 0));

                    let wl_modes: Vec<Mode> = drm_modes
                        .iter()
                        .map(|m| {
                            let size = m.size();
                            Mode {
                                size: (size.0 as i32, size.1 as i32).into(),
                                refresh: (m.vrefresh() * 1000) as i32,
                            }
                        })
                        .collect();

                    let descriptor = OutputDescriptor {
                        name: output_name,
                        physical_properties: PhysicalProperties {
                            size: (phys_w as i32, phys_h as i32).into(),
                            subpixel: wl_output::Subpixel::Unknown,
                            make: "Smithay".into(),
                            model: "Generic DRM".into(),
                        },
                    };

                    let wl_mode = handler.ask_for_output_mode(&descriptor, &wl_modes);

                    let mode_id = wl_modes.iter().position(|m| m == &wl_mode).unwrap();

                    let drm_mode = drm_modes[mode_id];

                    let mut surface =
                        match drm.create_surface(crtc, drm_mode, &[connector_info.handle()]) {
                            Ok(surface) => surface,
                            Err(err) => {
                                warn!("Failed to create drm surface: {}", err);
                                continue;
                            }
                        };
                    surface.link(signaler.clone());

                    let renderer_formats = Bind::<Dmabuf>::supported_formats(renderer)
                        .expect("Dmabuf renderer without formats");

                    let gbm_surface = match GbmBufferedSurface::new(
                        surface,
                        gbm.clone(),
                        renderer_formats,
                        slog_scope::logger(),
                    ) {
                        Ok(renderer) => renderer,
                        Err(err) => {
                            warn!("Failed to create rendering surface: {}", err);
                            continue;
                        }
                    };

                    let timer = Timer::new().unwrap();

                    entry.insert(Rc::new(RefCell::new(OutputSurfaceData {
                        output_descriptor: descriptor,

                        wl_mode,
                        wl_modes,

                        drm_modes: drm_modes.to_owned(),

                        surface: gbm_surface,
                        _render_timer: timer.handle(),
                        _connector_info: connector_info,
                        crtc,
                    })));
                    backends_order.push(crtc);

                    let inner = inner.clone();
                    handle
                        .insert_source(timer, move |(dev_id, crtc), _, handler| {
                            udev_render(handler, inner.clone(), dev_id, Some(crtc))
                        })
                        .unwrap();

                    break 'outer;
                }
            }
        }
    }

    ConnectorScanResult {
        backends,
        backends_order,
    }
}

/// Try to open the device
fn open_device(
    session: &mut AutoSession,
    device_id: dev_t,
    path: &Path,
) -> Option<(DrmDevice<SessionFd>, GbmDevice<SessionFd>)> {
    session
        .open(
            path,
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
        )
        .ok()
        .and_then(|fd| {
            let fd = SessionFd(fd);
            match (
                DrmDevice::new(fd.clone(), true, slog_scope::logger()),
                GbmDevice::new(fd),
            ) {
                (Ok(drm), Ok(gbm)) => Some((drm, gbm)),
                (Err(err), _) => {
                    warn!(
                        "Skipping device {:?}, because of drm error: {}",
                        device_id, err
                    );
                    None
                }
                (_, Err(err)) => {
                    // TODO try DumbBuffer allocator in this case
                    warn!(
                        "Skipping device {:?}, because of gbm error: {}",
                        device_id, err
                    );
                    None
                }
            }
        })
}

fn device_added<D>(
    handler: &mut D,
    handle: LoopHandle<'static, D>,
    inner: InnerRc,
    device_id: dev_t,
    path: PathBuf,
    session_signal: &Signaler<SessionSignal>,
) where
    D: BackendHandler + 'static,
{
    info!("Device Added {:?} : {:?}", device_id, path);

    let ret = open_device(&mut inner.borrow_mut().session, device_id, &path);

    // Try to open the device
    if let Some((mut drm, gbm)) = ret {
        let egl = match EGLDisplay::new(&gbm, slog_scope::logger()) {
            Ok(display) => display,
            Err(err) => {
                warn!(
                    "Skipping device {:?}, because of egl display error: {}",
                    device_id, err
                );
                return;
            }
        };

        let context = match EGLContext::new(&egl, slog_scope::logger()) {
            Ok(context) => context,
            Err(err) => {
                warn!(
                    "Skipping device {:?}, because of egl context error: {}",
                    device_id, err
                );
                return;
            }
        };

        let renderer = unsafe { Gles2Renderer::new(context, slog_scope::logger()).unwrap() };
        let renderer = Rc::new(RefCell::new(renderer));

        if path.canonicalize().ok() == inner.borrow().primary_gpu {
            info!("Initializing EGL Hardware Acceleration via {:?}", path);
            if renderer
                .borrow_mut()
                .bind_wl_display(&*inner.borrow().display.borrow())
                .is_ok()
            {
                info!("EGL hardware-acceleration enabled");
            }
        }

        let gbm = Rc::new(RefCell::new(gbm));
        let ConnectorScanResult {
            backends: outputs,
            backends_order: outputs_order,
        } = scan_connectors(
            handler,
            inner.clone(),
            handle.clone(),
            &mut drm,
            &gbm,
            &mut *renderer.borrow_mut(),
            session_signal,
        );

        {
            let mut inner = inner.borrow_mut();

            for output_handle in outputs_order {
                let output = {
                    let output_surface = outputs.get(&output_handle).unwrap();
                    let output_surface = output_surface.borrow();

                    let output_descriptor = &output_surface.output_descriptor;

                    let output = Output::new(
                        &mut inner.display.borrow_mut(),
                        handler.anodium_protocol(),
                        output_descriptor.clone(),
                        wl_output::Transform::Normal,
                        output_surface.wl_mode,
                        output_surface.wl_modes.clone(),
                    );

                    output.user_data().insert_if_missing(|| UdevOutputId {
                        crtc: output_surface.crtc,
                        device_id: drm.device_id(),
                    });

                    output
                };

                inner.outputs.push(output.clone());

                handler.output_created(output);
            }
        }

        let outputs = Rc::new(RefCell::new(outputs));

        let dev_id = drm.device_id();
        let restart_token = session_signal.register({
            let handle = handle.clone();
            let inner = inner.clone();

            move |signal| match signal {
                SessionSignal::ActivateSession | SessionSignal::ActivateDevice { .. } => {
                    let inner = inner.clone();
                    handle.insert_idle(move |handler| {
                        udev_render(handler, inner.clone(), dev_id, None)
                    });
                }
                _ => {}
            }
        });

        drm.link(session_signal.clone());
        let event_dispatcher = Dispatcher::new(drm, {
            let inner = inner.clone();
            move |event, _, handler| match event {
                DrmEvent::VBlank(crtc) => {
                    udev_render(handler, inner.clone(), dev_id, Some(crtc));
                }
                DrmEvent::Error(error) => {
                    error!("{:?}", error);
                }
            }
        });

        let _registration_token = handle
            .register_dispatcher(event_dispatcher.clone())
            .unwrap();

        trace!("Backends: {:?}", outputs.borrow().keys());
        for output in outputs.borrow_mut().values() {
            // render first frame
            trace!("Scheduling frame");
            schedule_initial_render(output.clone(), renderer.clone(), &handle);
        }

        inner.borrow_mut().udev_devices.insert(
            dev_id,
            UdevDeviceData {
                _restart_token: restart_token,
                // registration_token,
                // event_dispatcher,
                surfaces: outputs,
                renderer,
                // gbm,
                pointer_images: Vec::new(),
                dev_id,
            },
        );
    }
}

fn udev_render<D>(handler: &mut D, inner: InnerRc, dev_id: u64, crtc: Option<crtc::Handle>)
where
    D: BackendHandler + 'static,
{
    let inner = &mut *inner.borrow_mut();

    let device_backend = match inner.udev_devices.get_mut(&dev_id) {
        Some(backend) => backend,
        None => {
            error!("Trying to render on non-existent backend {}", dev_id);
            return;
        }
    };
    // setup two iterators on the stack, one over all surfaces for this backend, and
    // one containing only the one given as argument.
    // They make a trait-object to dynamically choose between the two
    let surfaces = device_backend.surfaces.borrow();
    let mut surfaces_iter = surfaces.iter();
    let mut option_iter = crtc
        .iter()
        .flat_map(|crtc| surfaces.get(crtc).map(|surface| (crtc, surface)));

    let to_render_iter: &mut dyn Iterator<Item = (&crtc::Handle, &Rc<RefCell<OutputSurfaceData>>)> =
        if crtc.is_some() {
            &mut option_iter
        } else {
            &mut surfaces_iter
        };

    for (&crtc, surface) in to_render_iter {
        // TODO get scale from the rendersurface when supporting HiDPI
        let frame = inner.pointer_image.get_image(1);
        let renderer = &mut *device_backend.renderer.borrow_mut();
        let pointer_images = &mut device_backend.pointer_images;
        let pointer_image = pointer_images
            .iter()
            .find_map(|(image, texture)| if image == &frame { Some(texture) } else { None })
            .cloned()
            .unwrap_or_else(|| {
                let image =
                    ImageBuffer::from_raw(frame.width, frame.height, &*frame.pixels_rgba).unwrap();
                let texture =
                    import_bitmap(renderer, &image, None).expect("Failed to import cursor bitmap");
                pointer_images.push((frame, texture.clone()));
                texture
            });

        let result = render_output_surface(
            handler,
            &inner.outputs,
            &mut *surface.borrow_mut(),
            renderer,
            device_backend.dev_id,
            crtc,
            &pointer_image,
        );

        if let Err(err) = result {
            warn!("Error during rendering: {:?}", err);
            let reschedule = match err {
                SwapBuffersError::AlreadySwapped => false,
                SwapBuffersError::TemporaryFailure(err) => !matches!(
                    err.downcast_ref::<DrmError>(),
                    Some(&DrmError::DeviceInactive)
                        | Some(&DrmError::Access {
                            source: drm::SystemError::PermissionDenied,
                            ..
                        })
                ),
                SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
            };

            if reschedule {
                // debug!("Rescheduling");

                // surface
                //     .borrow_mut()
                //     .render_timer
                //     .add_timeout(Duration::from_millis(1000), (device_backend.dev_id, crtc));
            }
        } else {
            // Send frame events so that client start drawing their next frame
            handler.send_frames();
        }
    }
}

fn schedule_initial_render<Data: 'static>(
    surface: Rc<RefCell<OutputSurfaceData>>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    evt_handle: &LoopHandle<'static, Data>,
) {
    let result = {
        let mut surface = surface.borrow_mut();
        let mut renderer = renderer.borrow_mut();
        initial_render(&mut surface.surface, &mut *renderer)
    };
    if let Err(err) = result {
        match err {
            SwapBuffersError::AlreadySwapped => {}
            SwapBuffersError::TemporaryFailure(err) => {
                // TODO dont reschedule after 3(?) retries
                warn!("Failed to submit page_flip: {}", err);
                let handle = evt_handle.clone();
                evt_handle
                    .insert_idle(move |_| schedule_initial_render(surface, renderer, &handle));
            }
            SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_output_surface<D>(
    handler: &mut D,
    outputs: &[Output],
    surface: &mut OutputSurfaceData,
    renderer: &mut Gles2Renderer,
    device_id: dev_t,
    crtc: crtc::Handle,
    pointer_image: &Gles2Texture,
) -> Result<(), SwapBuffersError>
where
    D: BackendHandler + 'static,
{
    surface.surface.frame_submitted()?;

    let output = outputs
        .iter()
        .find(|o| o.user_data().get::<UdevOutputId>() == Some(&UdevOutputId { device_id, crtc }));

    let output = if let Some(output) = output {
        output
    } else {
        // Somehow we got called with a non existing output
        return Ok(());
    };

    if output.pending_mode_change() {
        let current_mode = output.current_mode().unwrap();
        if let Some(drm_mode) = surface.drm_modes.iter().find(|m| {
            m.size() == (current_mode.size.w as u16, current_mode.size.h as u16)
                && m.vrefresh() == (current_mode.refresh / 1000) as u32
        }) {
            if let Err(err) = surface.surface.use_mode(*drm_mode) {
                error!("pending mode: {:?} failed: {:?}", current_mode, err);
            } else {
                surface.wl_mode = current_mode;
            }
        } else {
            error!("pending mode: {:?} not found in drm", current_mode);
        }
    }

    let (dmabuf, age) = surface.surface.next_buffer()?;
    renderer.bind(dmabuf)?;

    // and draw to our buffer
    handler
        .output_render(renderer, output, age as usize, Some(pointer_image))
        .ok();

    surface
        .surface
        .queue_buffer()
        .map_err(Into::<SwapBuffersError>::into)
}

fn initial_render(
    surface: &mut RenderSurface,
    renderer: &mut Gles2Renderer,
) -> Result<(), SwapBuffersError> {
    let (dmabuf, _age) = surface.next_buffer()?;
    renderer.bind(dmabuf)?;
    // Does not matter if we render an empty frame
    renderer
        .render((1, 1).into(), Transform::Normal, |_renderer, frame| {
            frame
                .clear(
                    [0.8, 0.8, 0.9, 1.0],
                    &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
                )
                .map_err(Into::<SwapBuffersError>::into)
        })
        .map_err(Into::<SwapBuffersError>::into)
        .and_then(|x| x.map_err(Into::<SwapBuffersError>::into))?;
    surface.queue_buffer()?;
    surface.reset_buffers();

    Ok(())
}
