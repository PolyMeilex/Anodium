use std::collections::{HashMap, HashSet};

use smithay::{
    backend::{
        allocator::{
            gbm::{self, GbmAllocator, GbmBufferFlags},
            Format as DrmFormat,
        },
        drm::{DrmDeviceFd, GbmBufferedSurface},
        egl::{EGLContext, EGLDevice, EGLDisplay},
        renderer::{gles2::Gles2Renderer, Bind, Frame, Renderer},
    },
    reexports::drm::control::{crtc, ModeTypeFlags},
    utils::Rectangle,
};

use super::drm_device::DrmDevice;

pub struct GbmDevice {
    gbm: gbm::GbmDevice<DrmDeviceFd>,
    allocator: GbmAllocator<DrmDeviceFd>,
    formats: HashSet<DrmFormat>,

    renderer: Gles2Renderer,
    pub out: HashMap<crtc::Handle, GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>>,
}

impl GbmDevice {
    pub fn new(device: &DrmDevice) -> Self {
        let gbm = gbm::GbmDevice::new(device.fd()).unwrap();

        let (formats, renderer) = {
            let display = EGLDisplay::new(gbm.clone()).unwrap();

            let _render_node = EGLDevice::device_for_display(&display)
                .ok()
                .and_then(|x| x.try_get_render_node().ok());

            let context = EGLContext::new(&display).unwrap();
            let formats = context.dmabuf_render_formats().clone();

            let renderer = unsafe { Gles2Renderer::new(context) }.unwrap();

            (formats, renderer)
        };

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );

        let mut out = HashMap::new();
        for (connector, crtc) in device.scan_crtcs() {
            let mode_id = connector
                .modes()
                .iter()
                .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(0);

            let drm_mode = connector.modes()[mode_id];

            let drm_surface = device
                .create_surface(crtc, drm_mode, &[connector.handle()])
                .unwrap();

            let mut gbm_surface =
                GbmBufferedSurface::new(drm_surface, allocator.clone(), formats.clone()).unwrap();

            gbm_surface.next_buffer().unwrap();
            gbm_surface.queue_buffer(None, ()).unwrap();

            out.insert(crtc, gbm_surface);
        }

        Self {
            gbm,
            allocator,
            formats,

            out,
            renderer,
        }
    }

    pub fn vblank(&mut self, crtc: crtc::Handle) {
        if let Some(surface) = self.out.get_mut(&crtc) {
            surface.frame_submitted().unwrap();

            let (dmabuf, age) = surface.next_buffer().unwrap();
            self.renderer.bind(dmabuf).unwrap();

            let mut frame = self
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
        //
    }
}
