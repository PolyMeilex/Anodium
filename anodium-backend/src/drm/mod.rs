use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    rc::Rc,
};

use anyhow::Result;
use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        drm::DrmNode,
        renderer::{
            gles2::Gles2Renderbuffer,
            multigpu::{egl::EglGlesBackend, GpuManager, MultiRenderer},
            ImportDma,
        },
        session::{auto::AutoSession, Session, Signal as SessionSignal},
    },
    reexports::{
        calloop::EventLoop,
        drm::control::crtc,
        wayland_server::{protocol::wl_output, DisplayHandle},
    },
    utils::signaling::SignalToken,
    wayland::{
        self,
        dmabuf::{DmabufGlobal, ImportError},
        output::{Mode as WlMode, PhysicalProperties},
    },
};

mod device;
use device::{Device, DrmDevice};

mod utils;

mod gpu;
use gpu::Gpu;

mod udev;

use crate::{BackendHandler, OutputId};

thread_local! {
    static OUTPUT_ID_MAP: RefCell<HashMap<OutputId, DrmOutputId>> = Default::default();
}

type DrmRenderer<'a> = MultiRenderer<'a, 'a, EglGlesBackend, EglGlesBackend, Gles2Renderbuffer>;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct DrmOutputId {
    drm_node: DrmNode,
    crtc: crtc::Handle,
}

impl DrmOutputId {
    fn output_id(&self) -> OutputId {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        OutputId {
            id: hasher.finish(),
        }
    }
}

pub struct DrmBackendState {
    gpus: HashMap<DrmNode, Gpu>,
    gpu_manager: Rc<RefCell<GpuManager<EglGlesBackend>>>,
    primary_gpu: DrmNode,
    _restart_token: SignalToken,
}

impl DrmBackendState {
    fn gpu(&mut self, node: &DrmNode) -> Option<&mut Gpu> {
        self.gpus.get_mut(node)
    }

    fn clear_all(&mut self) {
        for (_, gpu) in self.gpus.iter_mut() {
            if let Err(err) = gpu.clear_all(&mut self.gpu_manager.borrow_mut()) {
                error!("{}", err);
            }
        }
    }

    pub fn dmabuf_imported(
        &mut self,
        _dh: &DisplayHandle,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
    ) -> Result<(), ImportError> {
        self.gpu_manager
            .borrow_mut()
            .renderer::<Gles2Renderbuffer>(&self.primary_gpu, &self.primary_gpu)
            .and_then(|mut renderer| renderer.import_dmabuf(&dmabuf, None))
            .map(|_| ())
            .map_err(|_| ImportError::Failed)
    }

    pub fn update_mode(&mut self, output: &OutputId, mode: &wayland::output::Mode) {
        let id = OUTPUT_ID_MAP.with(|map| map.borrow().get(output).cloned());

        let output = id.and_then(|id| {
            let gpu = self.gpus.get_mut(&id.drm_node)?;
            gpu.outputs.get_mut(&id.crtc)
        });

        if let Some(output) = output {
            if let Err(err) = output.use_mode(mode) {
                error!("Gbm use mode error: {}", err);
            }
        }
    }
}

pub fn run_drm_backend<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: &DisplayHandle,
    handler: &mut D,
) -> Result<()>
where
    D: BackendHandler,
    D: 'static,
{
    // Init session
    let (mut session, notifier) = AutoSession::new(None).expect("Could not init session!");
    let session_signal = notifier.signaler();

    crate::libinput::init(event_loop.handle(), session.clone(), session_signal.clone());

    event_loop
        .handle()
        .insert_source(notifier, |_, _, _| {})
        .unwrap();

    let (primary_gpu_path, primary_gpu_node) = udev::primary_gpu(&session.seat());

    info!("Primary GPU: {:?}", primary_gpu_path);

    udev::init(event_loop.handle(), session.seat())?;

    let handle = event_loop.handle();
    let restart_token = session_signal.register(move |signal| match signal {
        SessionSignal::ActivateSession | SessionSignal::ActivateDevice { .. } => {
            handle.insert_idle(|data| {
                data.backend_state().drm().clear_all();
            });
        }
        SessionSignal::PauseSession | SessionSignal::PauseDevice { .. } => {}
    });

    let gpu = Gpu::new(
        event_loop.handle(),
        &mut session,
        session_signal,
        &primary_gpu_path,
        primary_gpu_node,
    )?;

    let outputs: Vec<_> = gpu.outputs.iter().map(|(crtc, _)| *crtc).collect();

    let mut gpus = HashMap::new();
    gpus.insert(primary_gpu_node, gpu);

    let gpu_manager = GpuManager::new(EglGlesBackend, None)?;
    let gpu_manager = Rc::new(RefCell::new(gpu_manager));

    handler.backend_state().init_drm(DrmBackendState {
        gpus,
        gpu_manager,
        primary_gpu: primary_gpu_node,
        _restart_token: restart_token,
    });

    // TODO: This should handle potential SwapBuffersError::TemporaryFailure errors and retry
    handler.backend_state().drm().clear_all();

    // Bind egl wl_display, uses c wayland libs
    // TODO: replace with implementation of wl_drm to keep the backwards compatibility, but with no c libs
    #[cfg(feature = "use_system_lib")]
    {
        use smithay::backend::renderer::ImportEgl;

        let state = handler.backend_state().drm();
        let mut gpu_manager = state.gpu_manager.borrow_mut();

        let mut renderer = gpu_manager
            .renderer::<Gles2Renderbuffer>(&state.primary_gpu, &state.primary_gpu)
            .unwrap();

        info!(
            "Trying to initialize EGL Hardware Acceleration via {:?}",
            state.primary_gpu
        );
        if renderer.bind_wl_display(display).is_ok() {
            info!("EGL hardware-acceleration enabled");
        }
    }

    // Init dmabuf_globabl for primary gpu
    let dmabuf_formats = {
        let state = handler.backend_state().drm();
        let mut gpu_manager = state.gpu_manager.borrow_mut();

        let renderer = gpu_manager
            .renderer::<Gles2Renderbuffer>(&state.primary_gpu, &state.primary_gpu)
            .unwrap();

        renderer.dmabuf_formats().cloned().collect::<Vec<_>>()
    };

    handler
        .dmabuf_state()
        .create_global::<D::WaylandState, _>(display, dmabuf_formats, None);

    for crtc in outputs {
        let id = DrmOutputId {
            drm_node: primary_gpu_node,
            crtc,
        };

        let mode = WlMode {
            size: (1920, 1080).into(),
            refresh: 60_000,
        };

        OUTPUT_ID_MAP.with(|map| map.borrow_mut().insert(id.output_id(), id));

        handler.output_created(crate::NewOutputDescriptor {
            id: id.output_id(),
            name: "".into(),
            physical_properties: PhysicalProperties {
                size: (1920, 1080).into(),
                subpixel: wl_output::Subpixel::Unknown,
                make: "".into(),
                model: "".into(),
            },
            prefered_mode: mode,
            possible_modes: vec![mode],
            transform: wl_output::Transform::Normal,
        })
    }

    handler.start_compositor();

    Ok(())
}
