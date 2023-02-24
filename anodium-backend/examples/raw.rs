use smithay::{
    backend::{
        allocator::gbm::{self, GbmAllocator, GbmBufferFlags},
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDevice, EGLDisplay},
        renderer::gles2::Gles2Renderer,
    },
    reexports::{
        calloop::{timer::Timer, EventLoop},
        drm::control::{connector::State as ConnectorState, crtc, Device as ControlDevice},
    },
    utils::DeviceFd,
};
use std::{
    fs::{File, OpenOptions},
    os::unix::{
        io::{AsRawFd, RawFd},
        prelude::OwnedFd,
    },
    rc::Rc,
    time::Duration,
};

#[derive(Clone)]
struct FdWrapper {
    file: Rc<File>,
}

impl AsRawFd for FdWrapper {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

fn main() {
    /*
     * Initialize the drm backend
     */

    // "Find" a suitable drm device
    let mut options = OpenOptions::new();
    options.read(true);
    options.write(true);

    let fd = DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(
        options.open("/dev/dri/card0").unwrap(),
    )));

    let (device, device_notifier) = DrmDevice::new(fd.clone(), true).unwrap();
    let gbm = gbm::GbmDevice::new(fd).unwrap();

    // Get a set of all modesetting resource handles (excluding planes):
    let res_handles = ControlDevice::resource_handles(&device).unwrap();

    // Use first connected connector
    let connector_info = res_handles
        .connectors()
        .iter()
        .map(|conn| device.get_connector(*conn, true).unwrap())
        .find(|conn| conn.state() == ConnectorState::Connected)
        .unwrap();

    dbg!(connector_info.current_encoder());

    // Use the first encoder
    let encoder = connector_info.encoders().iter().next().unwrap();
    let encoder_info = device.get_encoder(*encoder).unwrap();

    // use the connected crtc if any
    let crtc = encoder_info
        .crtc()
        // or use the first one that is compatible with the encoder
        .unwrap_or_else(|| res_handles.filter_crtcs(encoder_info.possible_crtcs())[0]);

    // Assuming we found a good connector and loaded the info into `connector_info`
    let mode = connector_info.modes()[0]; // Use first mode (usually highest resoltion, but in reality you should filter and sort and check and match with other connectors, if you use more then one.)

    let (formats, _renderer) = {
        let display = EGLDisplay::new(gbm.clone()).unwrap();

        let _render_node = EGLDevice::device_for_display(&display)
            .ok()
            .and_then(|x| x.try_get_render_node().ok());

        let context = EGLContext::new(&display).unwrap();
        let formats = context.dmabuf_render_formats().clone();

        let renderer = unsafe { Gles2Renderer::new(context) }.unwrap();

        (formats, renderer)
    };

    let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);

    // Initialize the hardware backend
    let drm_surface = device
        .create_surface(crtc, mode, &[connector_info.handle()])
        .unwrap();

    let mut gbm_surface =
        GbmBufferedSurface::<GbmAllocator<DrmDeviceFd>, ()>::new(drm_surface, allocator, formats)
            .unwrap();

    gbm_surface.next_buffer().unwrap();
    gbm_surface.queue_buffer(None, ()).unwrap();

    let mut vblank_handler = VBlankHandler {};

    /*
     * Register the DrmDevice on the EventLoop
     */
    let mut event_loop = EventLoop::<()>::try_new().unwrap();
    event_loop
        .handle()
        .insert_source(
            device_notifier,
            move |event, _: &mut _, _: &mut ()| match event {
                DrmEvent::VBlank(crtc) => vblank_handler.vblank(crtc),
                DrmEvent::Error(e) => panic!("{}", e),
            },
        )
        .unwrap();

    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(5)), |_, _, _| {
            panic!("Aborted");
        })
        .unwrap();

    // Run
    event_loop.run(None, &mut (), |_| {}).unwrap();
}

pub struct VBlankHandler {}

impl VBlankHandler {
    fn vblank(&mut self, _crtc: crtc::Handle) {
        dbg!("vblank");
        // {
        //     // Next buffer
        //     let next = self.swapchain.acquire().unwrap().unwrap();
        //     if next.userdata().get::<framebuffer::Handle>().is_none() {
        //         let fb = self.surface.add_framebuffer(next.handle(), 32, 32).unwrap();
        //         next.userdata().insert_if_missing(|| fb);
        //     }

        //     // now we could render to the mapping via software rendering.
        //     // this example just sets some grey color

        //     {
        //         let mut db = *next.handle();
        //         let mut mapping = self.surface.map_dumb_buffer(&mut db).unwrap();
        //         for x in mapping.as_mut() {
        //             *x = 128;
        //         }
        //     }
        //     self.current = next;
        // }

        // let fb = *self
        //     .current
        //     .userdata()
        //     .get::<framebuffer::Handle>()
        //     .unwrap();

        // let plane_state = PlaneState {
        //     handle: self.surface.plane(),
        //     config: Some(PlaneConfig {
        //         src: Rectangle::from_loc_and_size((0.0, 0.0), (100.0, 100.0)),
        //         dst: Rectangle::from_loc_and_size((0, 0), (100, 100)),
        //         transform: Transform::Normal,
        //         damage_clips: None,
        //         fb,
        //     }),
        // };

        // self.surface.page_flip([plane_state], true).unwrap();
    }
}
