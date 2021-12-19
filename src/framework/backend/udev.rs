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
            Bind, Frame, ImportDma, ImportEgl, Renderer, Transform,
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
                Device as ControlDevice,
            },
        },
        gbm::Device as GbmDevice,
        input::Libinput,
        nix::{fcntl::OFlag, sys::stat::dev_t},
        wayland_server::{protocol::wl_output, DispatchData, Display},
    },
    utils::signaling::{Linkable, SignalToken, Signaler},
    wayland::{
        dmabuf::init_dmabuf_global,
        output::{Mode, PhysicalProperties},
    },
};

use super::{BackendEvent, BackendRequest};
use crate::{framework, output_map::Output, render::renderer::RenderFrame, render::*};

struct Inner {
    cb: Box<dyn FnMut(BackendEvent, DispatchData)>,

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

pub fn run_udev<F, IF, D>(
    display: Rc<RefCell<Display>>,
    event_loop: &mut EventLoop<'static, D>,
    state: &mut D,
    mut session: AutoSession,
    notifier: AutoSessionNotifier,
    rx: channel::Channel<BackendRequest>,
    cb: F,
    mut input_cb: IF,
) -> Result<(), ()>
where
    F: FnMut(BackendEvent, DispatchData) + 'static,
    IF: FnMut(InputEvent<LibinputInputBackend>, DispatchData) + 'static,
    D: 'static,
{
    let log = slog_scope::logger();
    let mut ddata = DispatchData::wrap(state);

    let session_signal = notifier.signaler();

    /*
     * Initialize the compositor
     */

    let primary_gpu = primary_gpu(&session.seat()).unwrap_or_default();
    info!("Primary GPU: {:?}", primary_gpu);

    let inner = Rc::new(RefCell::new(Inner {
        cb: Box::new(cb),

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
        .insert_source(libinput_backend, move |event, _, state| {
            input_cb(event, DispatchData::wrap(state));
        })
        .unwrap();
    let _session_event_source = event_loop
        .handle()
        .insert_source(notifier, |(), &mut (), _anvil_state| {})
        .unwrap();

    for (dev, path) in udev_backend.device_list() {
        device_added(
            event_loop.handle(),
            inner.clone(),
            dev,
            path.into(),
            &session_signal,
            ddata.reborrow(),
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
        .insert_source(udev_backend, {
            let inner = inner.clone();
            move |event, _, state| match event {
                UdevEvent::Added { device_id, path } => device_added(
                    handle.clone(),
                    inner.clone(),
                    device_id,
                    path,
                    &session_signal,
                    DispatchData::wrap(state),
                ),
                UdevEvent::Changed { device_id } => {
                    error!("Udev device ({:?}) changed: unimplemented", device_id);
                }
                UdevEvent::Removed { device_id } => {
                    error!("Udev device ({:?}) removed: unimplemented", device_id);
                }
            }
        })
        .map_err(|e| -> IoError { e.into() })
        .unwrap();

    /*
     * Start XWayland and Wayland Socket
     */
    (inner.borrow_mut().cb)(BackendEvent::StartCompositor, ddata);

    // Cleanup stuff

    // event_loop.handle().remove(session_event_source);
    // event_loop.handle().remove(libinput_event_source);
    // event_loop.handle().remove(udev_event_source);

    Ok(())
}

pub type RenderSurface = GbmBufferedSurface<SessionFd>;

struct OutputSurfaceData {
    surface: RenderSurface,
    _render_timer: RenderTimerHandle,
    fps: fps_ticker::Fps,
    imgui: Option<imgui::SuspendedContext>,
    imgui_pipeline: imgui_smithay_renderer::Renderer,

    output_name: String,
    mode: Mode,
    connector_info: connector::Info,
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

fn scan_connectors<D: 'static>(
    inner: InnerRc,
    handle: LoopHandle<'static, D>,
    drm: &mut DrmDevice<SessionFd>,
    gbm: &GbmDevice<SessionFd>,
    renderer: &mut Gles2Renderer,
    signaler: &Signaler<SessionSignal>,
) -> (
    HashMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>,
    Vec<crtc::Handle>,
) {
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

                    let modes = connector_info.modes();
                    // let mode_id = anodium
                    //     .config
                    //     .configure_output(&output_name, modes)
                    //     .unwrap();

                    // let mode = modes.get(mode_id).unwrap();
                    let mode = modes[0];

                    info!("MODE: {:#?}", mode);

                    let mut surface =
                        match drm.create_surface(crtc, mode, &[connector_info.handle()]) {
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
                    let size = mode.size();
                    let mode = Mode {
                        size: (size.0 as i32, size.1 as i32).into(),
                        refresh: (mode.vrefresh() * 1000) as i32,
                    };

                    let timer = Timer::new().unwrap();

                    let mut imgui = imgui::Context::create();
                    {
                        imgui.set_ini_filename(None);
                        let io = imgui.io_mut();
                        io.display_framebuffer_scale = [1.0f32, 1.0f32];
                        io.display_size = [size.0 as f32, size.1 as f32];
                    }

                    let imgui_pipeline = renderer
                        .with_context(|_, gles| {
                            imgui_smithay_renderer::Renderer::new(gles, &mut imgui)
                        })
                        .unwrap();

                    entry.insert(Rc::new(RefCell::new(OutputSurfaceData {
                        surface: gbm_surface,
                        _render_timer: timer.handle(),
                        fps: fps_ticker::Fps::default(),
                        imgui: Some(imgui.suspend()),
                        imgui_pipeline,

                        output_name,
                        mode,
                        connector_info,
                        crtc,
                    })));
                    backends_order.push(crtc);

                    let inner = inner.clone();
                    handle
                        .insert_source(timer, move |(dev_id, crtc), _, ddata| {
                            udev_render(
                                inner.clone(),
                                dev_id,
                                Some(crtc),
                                DispatchData::wrap(ddata),
                            )
                        })
                        .unwrap();

                    break 'outer;
                }
            }
        }
    }

    (backends, backends_order)
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

fn device_added<D: 'static>(
    handle: LoopHandle<'static, D>,
    inner: InnerRc,
    device_id: dev_t,
    path: PathBuf,
    session_signal: &Signaler<SessionSignal>,
    mut ddata: DispatchData,
) {
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

        let (outputs, outputs_order) = scan_connectors(
            inner.clone(),
            handle.clone(),
            &mut drm,
            &gbm,
            &mut *renderer.borrow_mut(),
            session_signal,
        );

        {
            let mut inner = inner.borrow_mut();

            for output in outputs_order {
                let output = {
                    let output = outputs.get(&output).unwrap();

                    let display = inner.display.clone();
                    let display = &mut *display.borrow_mut();

                    let output = &*output.borrow();
                    let crtc = output.crtc;

                    let (phys_w, phys_h) = output.connector_info.size().unwrap_or((0, 0));

                    let output = Output::new(
                        &output.output_name,
                        Default::default(),
                        display,
                        PhysicalProperties {
                            size: (phys_w as i32, phys_h as i32).into(),
                            subpixel: wl_output::Subpixel::Unknown,
                            make: "Smithay".into(),
                            model: "Generic DRM".into(),
                        },
                        output.mode,
                        // TODO: output should always have a workspace
                        "Unknown".into(),
                        slog_scope::logger(),
                    );

                    output.userdata().insert_if_missing(|| UdevOutputId {
                        crtc,
                        device_id: drm.device_id(),
                    });

                    output
                };

                inner.outputs.push(output.clone());

                (inner.cb)(BackendEvent::OutputCreated { output }, ddata.reborrow());
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
                    handle.insert_idle(move |state| {
                        udev_render(inner.clone(), dev_id, None, DispatchData::wrap(state))
                    });
                }
                _ => {}
            }
        });

        drm.link(session_signal.clone());
        let event_dispatcher = Dispatcher::new(drm, {
            let inner = inner.clone();
            move |event, _, state| match event {
                DrmEvent::VBlank(crtc) => {
                    udev_render(inner.clone(), dev_id, Some(crtc), DispatchData::wrap(state));
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

fn udev_render(inner: InnerRc, dev_id: u64, crtc: Option<crtc::Handle>, mut ddata: DispatchData) {
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
                    import_bitmap(renderer, &image).expect("Failed to import cursor bitmap");
                pointer_images.push((frame, texture.clone()));
                texture
            });

        let result = render_output_surface(
            &mut inner.cb,
            &inner.outputs,
            &mut *surface.borrow_mut(),
            renderer,
            device_backend.dev_id,
            crtc,
            &pointer_image,
            ddata.reborrow(),
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
            (inner.cb)(BackendEvent::SendFrames, ddata.reborrow());
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
fn render_output_surface(
    cb: &mut Box<dyn FnMut(BackendEvent, DispatchData)>,
    outputs: &[Output],
    surface: &mut OutputSurfaceData,
    renderer: &mut Gles2Renderer,
    device_id: dev_t,
    crtc: crtc::Handle,
    pointer_image: &Gles2Texture,
    ddata: DispatchData,
) -> Result<(), SwapBuffersError> {
    surface.surface.frame_submitted()?;

    let output = outputs
        .iter()
        .find(|o| o.userdata().get::<UdevOutputId>() == Some(&UdevOutputId { device_id, crtc }));

    let output = if let Some(output) = output {
        output
    } else {
        // Somehow we got called with a non existing output
        return Ok(());
    };

    let (dmabuf, _age) = surface.surface.next_buffer()?;
    renderer.bind(dmabuf)?;
    // and draw to our buffer
    match renderer
        .render(
            surface.mode.size,
            Transform::Flipped180, // Scanout is rotated
            |renderer, frame| {
                let imgui = surface.imgui.take().unwrap();
                let mut imgui = imgui.activate().unwrap();
                let ui = imgui.frame();

                {
                    let mut frame = RenderFrame {
                        transform: Transform::Flipped180,
                        renderer,
                        frame,
                        imgui: &ui,
                    };

                    cb(
                        BackendEvent::OutputRender {
                            frame: &mut frame,
                            output,
                            pointer_image: Some(pointer_image),
                        },
                        ddata,
                    );
                }

                {
                    draw_fps(&ui, 1.0, surface.fps.avg());
                    let draw_data = ui.render();

                    renderer
                        .with_context(|_renderer, gles| {
                            surface
                                .imgui_pipeline
                                .render(Transform::Flipped180, gles, draw_data);
                        })
                        .unwrap();

                    surface.imgui = Some(imgui.suspend());
                }

                surface.fps.tick();
                Ok(())
            },
        )
        .map_err(Into::<SwapBuffersError>::into)
        .and_then(|x| x)
        .map_err(Into::<SwapBuffersError>::into)
    {
        Ok(()) => surface
            .surface
            .queue_buffer()
            .map_err(Into::<SwapBuffersError>::into),
        Err(err) => Err(err),
    }
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
                .clear([0.8, 0.8, 0.9, 1.0])
                .map_err(Into::<SwapBuffersError>::into)
        })
        .map_err(Into::<SwapBuffersError>::into)
        .and_then(|x| x.map_err(Into::<SwapBuffersError>::into))?;
    surface.queue_buffer()?;
    Ok(())
}
