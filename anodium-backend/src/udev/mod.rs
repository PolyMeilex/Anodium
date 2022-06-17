use std::{
    cell::RefCell,
    collections::hash_map::{DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    os::unix::io::{AsRawFd, RawFd},
    path::PathBuf,
    rc::Rc,
};

use image::ImageBuffer;

use indexmap::{map::Entry, IndexMap};
use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        drm::{DrmDevice, DrmError, DrmEvent, GbmBufferedSurface},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles2::{Gles2Renderer, Gles2Texture},
            Bind, Frame, ImportDma, Renderer,
        },
        session::{auto::AutoSession, Session, Signal as SessionSignal},
        udev::{primary_gpu, UdevBackend, UdevEvent},
        SwapBuffersError,
    },
    reexports::{
        calloop::{timer::Timer, EventLoop, LoopHandle},
        drm::{
            self,
            control::{connector, crtc, Device as _, Mode as DrmMode, ModeTypeFlags},
        },
        gbm::Device as GbmDevice,
        input::Libinput,
        nix::sys::stat::dev_t,
        wayland_server::{protocol::wl_output, DisplayHandle},
    },
    utils::{
        signaling::{Linkable, SignalToken, Signaler},
        Rectangle, Transform,
    },
    wayland::{
        self,
        output::{Mode, PhysicalProperties},
    },
};

use crate::{NewOutputDescriptor, OutputId};

use super::utils::{cursor, import_bitmap};
use super::BackendHandler;

mod device_map;
use device_map::Device;

pub struct UdevState {
    display: DisplayHandle,
    session: AutoSession,
    primary_gpu: Option<PathBuf>,
    pointer_image: cursor::Cursor,

    udev_devices: Rc<RefCell<HashMap<dev_t, UdevDeviceData>>>,
    // renderers: HashMap<usize, Gles2Renderer>,
    outputs: HashMap<OutputId, UdevOutputId>,
}

impl UdevState {
    pub fn change_vt(&mut self, vt: i32) {
        self.session.change_vt(vt).ok();
    }

    pub fn update_mode(&mut self, output: &OutputId, mode: &wayland::output::Mode) {
        let id = self.outputs.get(output).unwrap();

        let udev_devices = self.udev_devices.borrow();
        let device = udev_devices.get(&id.device_id).unwrap();

        let data = device.surfaces.get(&id.crtc).unwrap();

        let mut data = data.borrow_mut();
        let pos = data.wl_modes.iter().position(|m| m == mode);

        let mode = pos.and_then(|id| data._drm_modes.get(id)).copied();

        if let Some(mode) = mode {
            if let Err(err) = data.surface.use_mode(mode) {
                error!("Mode: {:?} failed: {:?}", mode, err);
            }
        } else {
            error!("Mode: {:?} not found in drm", mode);
        }
    }
}

#[derive(Clone, Copy)]
pub struct SessionFd(RawFd);
impl AsRawFd for SessionFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct UdevOutputId {
    device_id: dev_t,
    crtc: crtc::Handle,
}

impl UdevOutputId {
    fn output_id(&self) -> OutputId {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        OutputId {
            id: hasher.finish(),
        }
    }
}

// type RenderTimerHandle = TimerHandle<(u64, crtc::Handle)>;

pub fn run_udev<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: &DisplayHandle,
    handler: &mut D,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let (session, notifier) = AutoSession::new(None).expect("Could not init session!");

    let session_signal = notifier.signaler();

    /*
     * Initialize the compositor
     */

    let primary_gpu = primary_gpu(&session.seat()).unwrap_or_default();
    info!("Primary GPU: {:?}", primary_gpu);

    handler.backend_state().init_udev(UdevState {
        outputs: Default::default(),
        session: session.clone(),
        display: display.clone(),
        pointer_image: cursor::Cursor::load(),
        udev_devices: Default::default(),
        primary_gpu,
    });

    /*
     * Initialize the udev backend
     */
    let udev_backend = UdevBackend::new(session.seat(), None).map_err(|_| ())?;

    /*
     * Initialize libinput backend
     */
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<AutoSession>>(session.clone().into());
    libinput_context.udev_assign_seat(&session.seat()).unwrap();
    let mut libinput_backend = LibinputInputBackend::new(libinput_context, None);
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
            dev,
            path.into(),
            &session_signal,
        )
    }

    // init dmabuf support with format list from all gpus
    // TODO: We need to update this list, when the set of gpus changes
    // TODO2: This does not necessarily depend on egl, but mesa makes no use of it without wl_drm right now
    {
        let udev_devices = handler.backend_state().udev().udev_devices.borrow();
        let mut formats = Vec::new();
        for backend_data in udev_devices.values() {
            formats.extend(backend_data.renderer.borrow().dmabuf_formats().cloned());
        }

        // TODO(0.30)
        // init_dmabuf_global(
        //     &mut *display.borrow_mut(),
        //     formats,
        //     move |buffer, mut ddata| {
        //         let handler = ddata.get::<D>().unwrap();
        //         let udev_devices = handler.backend_state().udev().udev_devices.borrow();

        //         for backend_data in udev_devices.values() {
        //             if backend_data
        //                 .renderer
        //                 .borrow_mut()
        //                 .import_dmabuf(buffer, None)
        //                 .is_ok()
        //             {
        //                 return true;
        //             }
        //         }
        //         false
        //     },
        //     None,
        // );
    }

    let handle = event_loop.handle();
    let _udev_event_source = event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, handler| match event {
            UdevEvent::Added { device_id, path } => {
                device_added(handler, handle.clone(), device_id, path, &session_signal)
            }
            UdevEvent::Changed { device_id } => {
                error!("Udev device ({:?}) changed: unimplemented", device_id);
            }
            UdevEvent::Removed { device_id } => {
                error!("Udev device ({:?}) removed: unimplemented", device_id);
            }
        })
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
    output_name: String,
    physical_properties: PhysicalProperties,

    wl_mode: Mode,
    wl_modes: Vec<Mode>,

    _drm_modes: Vec<DrmMode>,

    surface: RenderSurface,
    _connector_info: connector::Info,
    crtc: crtc::Handle,
}

pub struct UdevDeviceData {
    _restart_token: SignalToken,
    surfaces: IndexMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>,
    pointer_images: Vec<(xcursor::parser::Image, Gles2Texture)>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    // gbm: GbmDevice<SessionFd>,
    // registration_token: RegistrationToken,
    // event_dispatcher: Dispatcher<'static, DrmDevice<SessionFd>, BackendState>,
    device_id: u64,
}

pub fn format_connector_name(interface: connector::Interface, interface_id: u32) -> String {
    let other_short_name;
    let interface_short_name = match interface {
        connector::Interface::DVII => "DVI-I",
        connector::Interface::DVID => "DVI-D",
        connector::Interface::DVIA => "DVI-A",
        connector::Interface::SVideo => "S-VIDEO",
        connector::Interface::DisplayPort => "DP",
        connector::Interface::HDMIA => "HDMI-A",
        connector::Interface::HDMIB => "HDMI-B",
        connector::Interface::EmbeddedDisplayPort => "eDP",
        other => {
            other_short_name = format!("{:?}", other);
            &other_short_name
        }
    };

    format!("{}-{}", interface_short_name, interface_id)
}

fn scan_connectors<D>(
    handle: LoopHandle<'static, D>,
    device: &mut Device<D>,
    signaler: &Signaler<SessionSignal>,
) -> IndexMap<crtc::Handle, Rc<RefCell<OutputSurfaceData>>>
where
    D: BackendHandler + 'static,
{
    let scan_result = device.scan_connectors();

    let drm = device.drm.as_source_ref();

    let mut backends = IndexMap::new();

    for (conn, crtc) in scan_result.map {
        let connector_info = drm.get_connector(conn).unwrap();

        if let Entry::Vacant(entry) = backends.entry(crtc) {
            info!(
                "Trying to setup connector {:?}-{} with crtc {:?}",
                connector_info.interface(),
                connector_info.interface_id(),
                crtc,
            );

            let output_name =
                format_connector_name(connector_info.interface(), connector_info.interface_id());

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

            let physical_properties = PhysicalProperties {
                size: (phys_w as i32, phys_h as i32).into(),
                subpixel: wl_output::Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Generic DRM".into(),
            };

            let mode_id = drm_modes
                .iter()
                .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(0);

            let wl_mode = wl_modes[mode_id];
            let drm_mode = drm_modes[mode_id];

            let mut surface = match drm.create_surface(crtc, drm_mode, &[conn]) {
                Ok(surface) => surface,
                Err(err) => {
                    warn!("Failed to create drm surface: {}", err);
                    continue;
                }
            };
            surface.link(signaler.clone());

            let renderer_formats = Bind::<Dmabuf>::supported_formats(&*device.renderer.borrow())
                .expect("Dmabuf renderer without formats");

            let gbm_surface = match GbmBufferedSurface::new(
                surface,
                device.gbm.clone(),
                renderer_formats,
                None,
            ) {
                Ok(renderer) => renderer,
                Err(err) => {
                    warn!("Failed to create rendering surface: {}", err);
                    continue;
                }
            };

            // let timer = Timer::new().unwrap();

            entry.insert(Rc::new(RefCell::new(OutputSurfaceData {
                output_name,
                physical_properties,

                wl_mode,
                wl_modes,

                _drm_modes: drm_modes.to_owned(),

                surface: gbm_surface,
                _connector_info: connector_info,
                crtc,
            })));

            // handle
            //     .insert_source(timer, move |(dev_id, crtc), _, handler| {
            //         udev_render(handler, dev_id, Some(crtc))
            //     })
            //     .unwrap();
        }
    }

    backends
}

fn device_added<D>(
    handler: &mut D,
    handle: LoopHandle<'static, D>,
    device_id: dev_t,
    path: PathBuf,
    session_signal: &Signaler<SessionSignal>,
) where
    D: BackendHandler + 'static,
{
    info!("Device Added {:?} : {:?}", device_id, path);

    let ret = Device::<D>::open(
        &mut handler.backend_state().udev().session,
        session_signal,
        &path,
        move |event, _, handler| match event {
            DrmEvent::VBlank(crtc) => {
                udev_render(handler, device_id, Some(crtc));
            }
            DrmEvent::Error(error) => {
                error!("{:?}", error);
            }
        },
    );

    match ret {
        Ok(mut device) => {
            let display = handler.backend_state().udev().display.clone();

            if path.canonicalize().ok() == handler.backend_state().udev().primary_gpu {
                info!("Initializing EGL Hardware Acceleration via {:?}", path);

                device.bind_wl_display(&display);
            }

            let outputs = scan_connectors(handle.clone(), &mut device, session_signal);

            let mut new_outputs = Vec::new();
            for (_, output_surface) in outputs.iter() {
                let (id, output) = {
                    let output_surface = output_surface.borrow();

                    let name = output_surface.output_name.clone();
                    let physical_properties = output_surface.physical_properties.clone();
                    let prefered_mode = output_surface.wl_mode;
                    let transform = wl_output::Transform::Normal;

                    let possible_modes = output_surface.wl_modes.clone();

                    let id = UdevOutputId {
                        crtc: output_surface.crtc,
                        device_id,
                    };

                    let output = NewOutputDescriptor {
                        id: id.output_id(),
                        name,
                        physical_properties,

                        prefered_mode,
                        possible_modes,

                        transform,
                    };

                    (id, output)
                };

                handler
                    .backend_state()
                    .udev()
                    .outputs
                    .insert(id.output_id(), id);

                new_outputs.push(output);
            }

            let restart_token = session_signal.register({
                let handle = handle.clone();

                move |signal| match signal {
                    SessionSignal::ActivateSession | SessionSignal::ActivateDevice { .. } => {
                        handle.insert_idle(move |handler| udev_render(handler, device_id, None));
                    }
                    _ => {}
                }
            });

            let _registration_token = handle.register_dispatcher(device.drm.clone()).unwrap();

            trace!("Backends: {:?}", outputs.keys());
            for output in outputs.values() {
                // render first frame
                trace!("Scheduling frame");
                schedule_initial_render(output.clone(), device.renderer.clone(), &handle);
            }

            let udev_devices = &handler.backend_state().udev().udev_devices;
            udev_devices.borrow_mut().insert(
                device_id,
                UdevDeviceData {
                    _restart_token: restart_token,
                    // registration_token,
                    // event_dispatcher,
                    surfaces: outputs,
                    renderer: device.renderer,
                    // gbm,
                    pointer_images: Vec::new(),
                    device_id,
                },
            );

            for output in new_outputs {
                handler.output_created(output);
            }
        }
        Err(err) => {
            error!("Skiping device '{}' because of: {}", device_id, err);
        }
    }
}

fn udev_render<D>(handler: &mut D, dev_id: u64, crtc: Option<crtc::Handle>)
where
    D: BackendHandler + 'static,
{
    let udev_devices = handler.backend_state().udev().udev_devices.clone();
    let mut udev_devices = udev_devices.borrow_mut();

    let device_backend = match udev_devices.get_mut(&dev_id) {
        Some(backend) => backend,
        None => {
            error!("Trying to render on non-existent backend {}", dev_id);
            return;
        }
    };
    // setup two iterators on the stack, one over all surfaces for this backend, and
    // one containing only the one given as argument.
    // They make a trait-object to dynamically choose between the two
    let surfaces = &device_backend.surfaces;
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
        let frame = handler.backend_state().udev().pointer_image.get_image(1);
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
            &mut *surface.borrow_mut(),
            renderer,
            device_backend.device_id,
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

fn schedule_initial_render<D>(
    surface: Rc<RefCell<OutputSurfaceData>>,
    renderer: Rc<RefCell<Gles2Renderer>>,
    evt_handle: &LoopHandle<'static, D>,
) where
    D: 'static,
    D: BackendHandler,
{
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

    let output_id = UdevOutputId { device_id, crtc }.output_id();

    let (dmabuf, age) = surface.surface.next_buffer()?;
    renderer.bind(dmabuf)?;

    // and draw to our buffer
    handler
        .output_render(renderer, &output_id, age as usize, Some(pointer_image))
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
                    &[Rectangle::from_loc_and_size((0.0, 0.0), (1.0, 1.0))],
                )
                .map_err(Into::<SwapBuffersError>::into)
        })
        .map_err(Into::<SwapBuffersError>::into)
        .and_then(|x| x.map_err(Into::<SwapBuffersError>::into))?;
    surface.queue_buffer()?;
    surface.reset_buffers();

    Ok(())
}
