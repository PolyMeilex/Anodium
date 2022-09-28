use std::{cell::RefCell, path::Path, rc::Rc};

use anyhow::Result;
use indexmap::IndexMap;
use smithay::{
    backend::{
        drm::{DrmEvent, DrmNode, GbmBufferedSurface},
        egl::{EGLContext, EGLDevice, EGLDisplay},
        renderer::{
            gles2::Gles2Renderbuffer,
            multigpu::{egl::EglGlesBackend, GpuManager},
            Bind, Frame, ImportMem, Renderer,
        },
        session::{auto::AutoSession, Signal as SessionSignal},
    },
    output::Mode as WlMode,
    reexports::{
        calloop::LoopHandle,
        drm::control::{connector, crtc, Device as _, ModeTypeFlags},
        gbm::Device as GbmDevice,
    },
    utils::{
        signaling::{Linkable, Signaler},
        Rectangle,
    },
};

use super::{utils, Device, DrmDevice, DrmOutputId, DrmRenderer};
use crate::BackendHandler;

pub struct Gpu {
    drm: DrmDevice,
    drm_node: DrmNode,
    pub outputs: IndexMap<crtc::Handle, GpuConnector>,
}

impl Gpu {
    pub fn new<D>(
        event_loop: LoopHandle<'static, D>,
        session: &mut AutoSession,
        session_signal: Signaler<SessionSignal>,
        path: &Path,
        drm_node: DrmNode,
    ) -> Result<Gpu>
    where
        D: BackendHandler,
        D: 'static,
    {
        let device = Device::open(session, path)?;

        let mut drm = DrmDevice::new(
            &event_loop,
            device.clone(),
            move |event, _, handler: &mut D| match event {
                smithay::backend::drm::DrmEvent::VBlank(crtc) => {
                    if let Err(err) = Gpu::drm_vblank(drm_node, crtc, handler) {
                        error!("VBlank error: {}", err);
                    }
                }
                DrmEvent::Error(err) => error!("DrmEvent error: {}", err),
            },
        )?;

        drm.inner_mut().link(session_signal.clone());

        let gbm = GbmDevice::new(device)?;
        let gbm = Rc::new(RefCell::new(gbm));

        let res = drm.scan_connectors();
        info!("connectors: {:#?}", &res);

        let formats = {
            let display = unsafe { EGLDisplay::new(&*gbm.borrow(), None).unwrap() };

            EGLDevice::device_for_display(&display)
                .ok()
                .and_then(|x| x.try_get_render_node().ok());

            let context = EGLContext::new(&display, None).unwrap();

            context.dmabuf_render_formats().clone()
        };

        let mut outputs: IndexMap<crtc::Handle, GpuConnector> = IndexMap::new();

        for (connector, crtc) in res.map {
            let drm = drm.inner();

            let connector_info = drm.get_connector(connector, false).unwrap();

            let connector_name = utils::format_connector_name(
                connector_info.interface(),
                connector_info.interface_id(),
            );

            info!(
                "Trying to setup connector {:?}-{} with crtc {:?} ({})",
                connector_info.interface(),
                connector_info.interface_id(),
                crtc,
                connector_name,
            );

            let drm_modes = connector_info.modes();

            let wl_modes: Vec<WlMode> = drm_modes
                .iter()
                .map(|mode| WlMode {
                    size: (mode.size().0 as i32, mode.size().1 as i32).into(),
                    refresh: (mode.vrefresh() * 1000) as i32,
                })
                .collect();

            let mode_id = drm_modes
                .iter()
                .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(0);

            let drm_mode = drm_modes[mode_id];

            let mut drm_surface = drm.create_surface(crtc, drm_mode, &[connector])?;
            drm_surface.link(session_signal.clone());

            let gbm_surface =
                GbmBufferedSurface::new(drm_surface, gbm.clone(), formats.clone(), None)?;

            outputs.insert(
                crtc,
                GpuConnector {
                    connector,
                    gbm_surface,
                    drm_modes: drm_modes.to_vec(),
                    wl_modes,
                },
            );
        }

        Ok(Gpu {
            drm,
            drm_node,
            outputs,
        })
    }

    pub fn clear_all(&mut self, renderer: &mut GpuManager<EglGlesBackend>) -> Result<bool> {
        let mut is_err = false;

        let mut renderer = renderer.renderer(&self.drm_node, &self.drm_node)?;

        for (_, output) in self.outputs.iter_mut() {
            is_err |= output.clear(&mut renderer).is_err();
        }

        Ok(!is_err)
    }

    pub fn drm_vblank<D>(drm_node: DrmNode, crtc: crtc::Handle, handler: &mut D) -> Result<()>
    where
        D: BackendHandler,
    {
        let primary_gpu = handler.backend_state().drm().primary_gpu;

        let gpu_manager = handler.backend_state().drm().gpu_manager.clone();
        let mut gpu_manager = gpu_manager.borrow_mut();

        let mut renderer = gpu_manager.renderer::<Gles2Renderbuffer>(&primary_gpu, &drm_node)?;

        let age = {
            let state = handler.backend_state().drm();

            let gpu = &mut state.gpu(&drm_node).unwrap();
            let output = gpu.outputs.get_mut(&crtc).unwrap();

            output.gbm_surface.frame_submitted()?;

            let (dmabuf, age) = output.gbm_surface.next_buffer()?;
            renderer.bind(dmabuf).unwrap();

            age
        };

        let pointer_image = {
            let backend_state = handler.backend_state().drm();

            let frame = backend_state.pointer_image.get_image(1);

            backend_state
                .pointer_images
                .iter()
                .find_map(|(image, texture)| if image == &frame { Some(texture) } else { None })
                .cloned()
                .unwrap_or_else(|| {
                    let texture = renderer
                        .as_mut()
                        .import_memory(
                            &frame.pixels_rgba,
                            (frame.width as i32, frame.height as i32).into(),
                            false,
                        )
                        .expect("Failed to import cursor bitmap");
                    backend_state.pointer_images.push((frame, texture.clone()));
                    texture
                })
        };

        let output_id = DrmOutputId { drm_node, crtc }.output_id();
        handler.output_render(
            renderer.as_mut(),
            &output_id,
            age as usize,
            Some(&pointer_image),
        )?;

        handler.send_frames(&output_id);

        handler
            .backend_state()
            .drm()
            .gpu(&drm_node)
            .unwrap()
            .outputs
            .get_mut(&crtc)
            .unwrap()
            .gbm_surface
            .queue_buffer()?;

        Ok(())
    }

    /// Udev changed event
    pub fn changed_event<D>(drm_node: DrmNode, handler: &mut D)
    where
        D: BackendHandler,
    {
        if let Some(gpu) = handler.backend_state().drm().gpu(&drm_node) {
            let scan = gpu.drm.scan_connectors();
            info!("connectors: {:#?}", &scan);

            let removed: Vec<crtc::Handle> = scan
                .removed
                .iter()
                .flat_map(|connector| {
                    gpu.outputs
                        .iter()
                        .filter(|(_, o)| o.connector == *connector)
                        .map(|(crtc, _)| *crtc)
                })
                .collect();

            for crtc in removed {
                let id = super::DrmOutputId { drm_node, crtc };
                handler.output_removed(&id.output_id());
            }

            for _connector in scan.added {}
        }
    }
}

pub struct GpuConnector {
    connector: connector::Handle,
    gbm_surface: GbmBufferedSurface<Rc<RefCell<GbmDevice<Device>>>, Device>,
    drm_modes: Vec<smithay::reexports::drm::control::Mode>,
    wl_modes: Vec<WlMode>,
}

impl GpuConnector {
    pub fn clear(&mut self, renderer: &mut DrmRenderer) -> Result<()> {
        self.gbm_surface.frame_submitted()?;

        let (dmabuf, _) = self.gbm_surface.next_buffer()?;
        renderer.bind(dmabuf)?;

        renderer.render(
            (i32::MAX, i32::MAX).into(),
            smithay::utils::Transform::Normal,
            |_, frame| {
                frame.clear(
                    [0.2, 0.2, 0.2, 1.0],
                    &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
                )
            },
        )??;

        self.gbm_surface.queue_buffer()?;
        self.reset_buffers();

        Ok(())
    }

    /// Reset age of buffers
    pub fn reset_buffers(&mut self) {
        self.gbm_surface.reset_buffers();
    }

    pub fn use_mode(&mut self, mode: &WlMode) -> Result<()> {
        let mode = self
            .wl_modes
            .iter()
            .position(|m| m == mode)
            .and_then(|id| self.drm_modes.get(id));

        if let Some(mode) = mode {
            self.gbm_surface.use_mode(*mode)?;
        }

        Ok(())
    }
}
