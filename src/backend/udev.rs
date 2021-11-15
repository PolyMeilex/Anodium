use std::path::Path;
use std::{
    cell::RefCell,
    collections::hash_map::{Entry, HashMap},
    io::Error as IoError,
    os::unix::io::{AsRawFd, RawFd},
    path::PathBuf,
    rc::Rc,
};

use image::ImageBuffer;
use slog::Logger;

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        drm::{DrmDevice, DrmError, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles2::{Gles2Renderer, Gles2Texture},
            Bind, Frame, Renderer, Transform,
        },
        session::{auto::AutoSession, Session, Signal as SessionSignal},
        udev::{UdevBackend, UdevEvent},
        SwapBuffersError,
    },
    reexports::{
        calloop::{
            timer::{Timer, TimerHandle},
            Dispatcher, EventLoop, LoopHandle, RegistrationToken,
        },
        drm::{
            self,
            control::{
                connector::{Info as ConnectorInfo, State as ConnectorState},
                crtc,
                encoder::Info as EncoderInfo,
                Device as ControlDevice,
            },
        },
        gbm::Device as GbmDevice,
        input::Libinput,
        nix::{fcntl::OFlag, sys::stat::dev_t},
        wayland_server::{protocol::wl_output, Display},
    },
    utils::{
        signaling::{Linkable, SignalToken, Signaler},
        Logical, Point,
    },
    wayland::{
        output::{Mode, PhysicalProperties},
        seat::CursorImageStatus,
    },
};
use smithay::{
    backend::{
        drm::DevPath,
        renderer::{ImportDma, ImportEgl},
        udev::primary_gpu,
    },
    wayland::dmabuf::init_dmabuf_global,
};

use super::session::AnodiumSession;
use crate::state::Anodium;
use crate::{render::renderer::RenderFrame, render::*, state::BackendState};

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

pub fn run_udev(
    display: Rc<RefCell<Display>>,
    event_loop: &mut EventLoop<'static, BackendState>,
    log: Logger,
) -> Result<BackendState, ()> {
    /*
     * Initialize session
     */
    let (session, notifier) = AutoSession::new(log.clone()).ok_or(())?;

    let session_signal = notifier.signaler();
    let session = AnodiumSession::new_udev(session);

    /*
     * Initialize the compositor
     */

    let mut state = BackendState::init(display.clone(), event_loop.handle(), session, log.clone());
    state.primary_gpu = primary_gpu(&state.anodium.seat_name).unwrap_or_default();
    info!("Primary GPU: {:?}", state.primary_gpu);

    /*
     * Initialize the udev backend
     */
    let udev_backend =
        UdevBackend::new(state.anodium.seat_name.clone(), log.clone()).map_err(|_| ())?;

    /*
     * Initialize libinput backend
     */
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<AnodiumSession>>(
        state.anodium.session.clone().into(),
    );
    libinput_context
        .udev_assign_seat(&state.anodium.seat_name)
        .unwrap();
    let mut libinput_backend = LibinputInputBackend::new(libinput_context, log.clone());
    libinput_backend.link(session_signal.clone());

    /*
     * Bind all our objects that get driven by the event loop
     */
    let _libinput_event_source = event_loop
        .handle()
        .insert_source(libinput_backend, move |event, _, state| {
            state.anodium.process_input_event(event);
        })
        .unwrap();
    let _session_event_source = event_loop
        .handle()
        .insert_source(notifier, |(), &mut (), _anvil_state| {})
        .unwrap();
    for (dev, path) in udev_backend.device_list() {
        state.device_added(dev, path.into(), &session_signal)
    }

    // init dmabuf support with format list from all gpus
    // TODO: We need to update this list, when the set of gpus changes
    // TODO2: This does not necessarily depend on egl, but mesa makes no use of it without wl_drm right now
    {
        let mut formats = Vec::new();
        for backend_data in state.udev_devices.values() {
            formats.extend(backend_data.renderer.borrow().dmabuf_formats().cloned());
        }

        init_dmabuf_global(
            &mut *display.borrow_mut(),
            formats,
            |buffer, mut ddata| {
                let anvil_state = ddata.get::<BackendState>().unwrap();
                for backend_data in anvil_state.udev_devices.values() {
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
            },
            log.clone(),
        );
    }

    let _udev_event_source = event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, state| match event {
            UdevEvent::Added { device_id, path } => {
                state.device_added(device_id, path, &session_signal)
            }
            UdevEvent::Changed { device_id } => state.device_changed(device_id, &session_signal),
            UdevEvent::Removed { device_id } => state.device_removed(device_id),
        })
        .map_err(|e| -> IoError { e.into() })
        .unwrap();

    /*
     * Start XWayland if supported
     */
    #[cfg(feature = "xwayland")]
    state.start_xwayland();

    // Cleanup stuff

    // event_loop.handle().remove(session_event_source);
    // event_loop.handle().remove(libinput_event_source);
    // event_loop.handle().remove(udev_event_source);

    Ok(state)
}

pub type RenderSurface = GbmBufferedSurface<SessionFd>;

struct OutputSurfaceData {
    surface: RenderSurface,
    _render_timer: RenderTimerHandle,
    fps: fps_ticker::Fps,
    imgui: Option<imgui::SuspendedContext>,
    imgui_pipeline: imgui_smithay_renderer::Renderer,
}

pub struct UdevDeviceData {
    _restart_token: SignalToken,
    surfaces: Rc<RefCell<HashMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>>>,
    pointer_images: Vec<(xcursor::parser::Image, Gles2Texture)>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    gbm: GbmDevice<SessionFd>,
    registration_token: RegistrationToken,
    event_dispatcher: Dispatcher<'static, DrmDevice<SessionFd>, BackendState>,
    dev_id: u64,
}

fn scan_connectors(
    handle: LoopHandle<'static, BackendState>,
    drm: &mut DrmDevice<SessionFd>,
    gbm: &GbmDevice<SessionFd>,
    renderer: &mut Gles2Renderer,
    anodium: &mut Anodium,
    signaler: &Signaler<SessionSignal>,
    logger: &::slog::Logger,
) -> HashMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>> {
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
                    let mode_id = anodium
                        .config
                        .configure_output(&output_name, modes)
                        .unwrap();

                    let mode = modes.get(mode_id).unwrap();

                    info!("MODE: {:#?}", mode);

                    let mut surface =
                        match drm.create_surface(crtc, *mode, &[connector_info.handle()]) {
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
                        logger.clone(),
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

                    let (phys_w, phys_h) = connector_info.size().unwrap_or((0, 0));

                    anodium.add_output(
                        &output_name,
                        PhysicalProperties {
                            size: (phys_w as i32, phys_h as i32).into(),
                            subpixel: wl_output::Subpixel::Unknown,
                            make: "Smithay".into(),
                            model: "Generic DRM".into(),
                        },
                        mode,
                        |output| {
                            output.userdata().insert_if_missing(|| UdevOutputId {
                                crtc,
                                device_id: drm.device_id(),
                            });
                        },
                    );

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
                    })));

                    handle
                        .insert_source(timer, |(dev_id, crtc), _, state| {
                            state.udev_render(dev_id, Some(crtc))
                        })
                        .unwrap();

                    break 'outer;
                }
            }
        }
    }

    backends
}

impl BackendState {
    /// Try to open the device
    fn open_device(
        &mut self,
        device_id: dev_t,
        path: &Path,
    ) -> Option<(DrmDevice<SessionFd>, GbmDevice<SessionFd>)> {
        self.anodium
            .session
            .open(
                path,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            )
            .ok()
            .and_then(|fd| {
                let fd = SessionFd(fd);
                match (
                    DrmDevice::new(fd.clone(), true, self.log.clone()),
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

    fn device_added(
        &mut self,
        device_id: dev_t,
        path: PathBuf,
        session_signal: &Signaler<SessionSignal>,
    ) {
        info!("Device Added {:?} : {:?}", device_id, path);

        // Try to open the device
        if let Some((mut drm, gbm)) = self.open_device(device_id, &path) {
            let egl = match EGLDisplay::new(&gbm, self.log.clone()) {
                Ok(display) => display,
                Err(err) => {
                    warn!(
                        "Skipping device {:?}, because of egl display error: {}",
                        device_id, err
                    );
                    return;
                }
            };

            let context = match EGLContext::new(&egl, self.log.clone()) {
                Ok(context) => context,
                Err(err) => {
                    warn!(
                        "Skipping device {:?}, because of egl context error: {}",
                        device_id, err
                    );
                    return;
                }
            };

            let renderer = unsafe { Gles2Renderer::new(context, self.log.clone()).unwrap() };
            let renderer = Rc::new(RefCell::new(renderer));

            if path.canonicalize().ok() == self.primary_gpu {
                info!("Initializing EGL Hardware Acceleration via {:?}", path);
                if renderer
                    .borrow_mut()
                    .bind_wl_display(&*self.anodium.display.borrow())
                    .is_ok()
                {
                    info!("EGL hardware-acceleration enabled");
                }
            }

            let outputs = Rc::new(RefCell::new(scan_connectors(
                self.handle.clone(),
                &mut drm,
                &gbm,
                &mut *renderer.borrow_mut(),
                &mut self.anodium,
                session_signal,
                &self.log,
            )));

            let dev_id = drm.device_id();
            let handle = self.handle.clone();
            let restart_token = session_signal.register(move |signal| match signal {
                SessionSignal::ActivateSession | SessionSignal::ActivateDevice { .. } => {
                    handle.insert_idle(move |anvil_state| anvil_state.udev_render(dev_id, None));
                }
                _ => {}
            });

            drm.link(session_signal.clone());
            let event_dispatcher = Dispatcher::new(
                drm,
                move |event, _, anvil_state: &mut BackendState| match event {
                    DrmEvent::VBlank(crtc) => anvil_state.udev_render(dev_id, Some(crtc)),
                    DrmEvent::Error(error) => {
                        error!("{:?}", error);
                    }
                },
            );
            let registration_token = self
                .handle
                .register_dispatcher(event_dispatcher.clone())
                .unwrap();

            trace!("Backends: {:?}", outputs.borrow().keys());
            for output in outputs.borrow_mut().values() {
                // render first frame
                trace!("Scheduling frame");
                schedule_initial_render(
                    output.clone(),
                    renderer.clone(),
                    &self.handle,
                    self.log.clone(),
                );
            }

            self.udev_devices.insert(
                dev_id,
                UdevDeviceData {
                    _restart_token: restart_token,
                    registration_token,
                    event_dispatcher,
                    surfaces: outputs,
                    renderer,
                    gbm,
                    pointer_images: Vec::new(),
                    dev_id,
                },
            );
        }
    }

    fn device_changed(&mut self, device: dev_t, session_signal: &Signaler<SessionSignal>) {
        //quick and dirty, just re-init all backends
        if let Some(ref mut backend_data) = self.udev_devices.get_mut(&device) {
            let logger = self.log.clone();
            let loop_handle = self.handle.clone();
            let signaler = session_signal.clone();

            self.anodium.retain_outputs(|output| {
                output
                    .userdata()
                    .get::<UdevOutputId>()
                    .map(|id| id.device_id != device)
                    .unwrap_or(true)
            });

            let mut source = backend_data.event_dispatcher.as_source_mut();
            let mut backends = backend_data.surfaces.borrow_mut();
            *backends = scan_connectors(
                self.handle.clone(),
                &mut *source,
                &backend_data.gbm,
                &mut *backend_data.renderer.borrow_mut(),
                &mut self.anodium,
                &signaler,
                &logger,
            );

            for renderer in backends.values() {
                let logger = logger.clone();
                // render first frame
                schedule_initial_render(
                    renderer.clone(),
                    backend_data.renderer.clone(),
                    &loop_handle,
                    logger,
                );
            }
        }
    }

    fn device_removed(&mut self, device: dev_t) {
        // drop the backends on this side
        if let Some(backend_data) = self.udev_devices.remove(&device) {
            // drop surfaces
            backend_data.surfaces.borrow_mut().clear();
            debug!("Surfaces dropped");

            self.anodium.retain_outputs(|output| {
                output
                    .userdata()
                    .get::<UdevOutputId>()
                    .map(|id| id.device_id != device)
                    .unwrap_or(true)
            });

            let _device = self.handle.remove(backend_data.registration_token);
            let _device = backend_data.event_dispatcher.into_source_inner();

            // don't use hardware acceleration anymore, if this was the primary gpu
            if _device.dev_path().and_then(|path| path.canonicalize().ok()) == self.primary_gpu {
                backend_data.renderer.borrow_mut().unbind_wl_display();
            }
            debug!("Dropping device");
        }
    }

    // If crtc is `Some()`, render it, else render all crtcs
    fn udev_render(&mut self, dev_id: u64, crtc: Option<crtc::Handle>) {
        let device_backend = match self.udev_devices.get_mut(&dev_id) {
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

        let to_render_iter: &mut dyn Iterator<
            Item = (&crtc::Handle, &Rc<RefCell<OutputSurfaceData>>),
        > = if crtc.is_some() {
            &mut option_iter
        } else {
            &mut surfaces_iter
        };

        for (&crtc, surface) in to_render_iter {
            // TODO get scale from the rendersurface when supporting HiDPI
            let frame = self.pointer_image.get_image(
                1, /*scale*/
                self.anodium.start_time.elapsed().as_millis() as u32,
            );
            let renderer = &mut *device_backend.renderer.borrow_mut();
            let pointer_images = &mut device_backend.pointer_images;
            let pointer_image = pointer_images
                .iter()
                .find_map(|(image, texture)| if image == &frame { Some(texture) } else { None })
                .cloned()
                .unwrap_or_else(|| {
                    let image =
                        ImageBuffer::from_raw(frame.width, frame.height, &*frame.pixels_rgba)
                            .unwrap();
                    let texture =
                        import_bitmap(renderer, &image).expect("Failed to import cursor bitmap");
                    pointer_images.push((frame, texture.clone()));
                    texture
                });

            let result = self.anodium.render_output_surface(
                &mut *surface.borrow_mut(),
                renderer,
                device_backend.dev_id,
                crtc,
                &pointer_image,
                &mut self.cursor_status.lock().unwrap(),
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
                let time = self.anodium.start_time.elapsed().as_millis() as u32;
                self.anodium.send_frames(time);
            }
        }
    }
}

fn schedule_initial_render<Data: 'static>(
    surface: Rc<RefCell<OutputSurfaceData>>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    evt_handle: &LoopHandle<'static, Data>,
    logger: ::slog::Logger,
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
                evt_handle.insert_idle(move |_| {
                    schedule_initial_render(surface, renderer, &handle, logger)
                });
            }
            SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
        }
    }
}

impl Anodium {
    #[allow(clippy::too_many_arguments)]
    fn render_output_surface(
        &mut self,
        surface: &mut OutputSurfaceData,
        renderer: &mut Gles2Renderer,
        device_id: dev_t,
        crtc: crtc::Handle,
        pointer_image: &Gles2Texture,
        cursor_status: &mut CursorImageStatus,
    ) -> Result<(), SwapBuffersError> {
        surface.surface.frame_submitted()?;

        let output = self
            .desktop_layout
            .borrow_mut()
            .output_map
            .find(|o| o.userdata().get::<UdevOutputId>() == Some(&UdevOutputId { device_id, crtc }))
            .map(|output| (output.geometry(), output.scale(), output.current_mode()));

        let (output_geometry, output_scale, mode) = if let Some((geometry, scale, mode)) = output {
            (geometry, scale, mode)
        } else {
            // Somehow we got called with a non existing output
            return Ok(());
        };

        let dmabuf = surface.surface.next_buffer()?;
        renderer.bind(dmabuf)?;
        // and draw to our buffer
        match renderer
            .render(
                mode.size,
                Transform::Flipped180, // Scanout is rotated
                |renderer, frame| {
                    let mut frame = RenderFrame {
                        transform: Transform::Flipped180,
                        renderer,
                        frame,
                    };

                    self.render(&mut frame, (output_geometry, output_scale))?;

                    let imgui_pipeline = &surface.imgui_pipeline;
                    let imgui = surface.imgui.take().unwrap();

                    let mut imgui = imgui.activate().unwrap();
                    let ui = imgui.frame();
                    draw_fps(&ui, 1.0, surface.fps.avg());
                    let draw_data = ui.render();

                    frame
                        .renderer
                        .with_context(|_renderer, gles| {
                            imgui_pipeline.render(Transform::Flipped180, gles, draw_data);
                        })
                        .unwrap();

                    surface.imgui = Some(imgui.suspend());

                    // set cursor
                    if output_geometry
                        .to_f64()
                        .contains(self.input_state.pointer_location)
                    {
                        let (ptr_x, ptr_y) = self.input_state.pointer_location.into();
                        let relative_ptr_location =
                            Point::<i32, Logical>::from((ptr_x as i32, ptr_y as i32))
                                - output_geometry.loc;
                        // draw the cursor as relevant
                        {
                            // reset the cursor if the surface is no longer alive
                            let mut reset = false;
                            if let CursorImageStatus::Image(ref surface) = *cursor_status {
                                reset = !surface.as_ref().is_alive();
                            }
                            if reset {
                                *cursor_status = CursorImageStatus::Default;
                            }

                            if let CursorImageStatus::Image(ref wl_surface) = *cursor_status {
                                draw_cursor(
                                    &mut frame,
                                    wl_surface,
                                    relative_ptr_location,
                                    output_scale,
                                )?;
                            } else {
                                frame.render_texture_at(
                                    pointer_image,
                                    relative_ptr_location
                                        .to_f64()
                                        .to_physical(output_scale as f64)
                                        .to_i32_round(),
                                    1,
                                    output_scale as f64,
                                    Transform::Normal,
                                    1.0,
                                )?;
                            }
                        }
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
}

fn initial_render(
    surface: &mut RenderSurface,
    renderer: &mut Gles2Renderer,
) -> Result<(), SwapBuffersError> {
    let dmabuf = surface.next_buffer()?;
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
