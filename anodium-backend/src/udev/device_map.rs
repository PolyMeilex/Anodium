use std::{cell::RefCell, io, path::Path, rc::Rc};

use calloop::Dispatcher;
use smithay::{
    backend::{
        drm::{DrmDevice, DrmError, DrmEvent, DrmEventMetadata},
        egl::{self, EGLContext, EGLDisplay},
        renderer::{
            gles2::{Gles2Error, Gles2Renderer},
            ImportEgl,
        },
        session::{
            auto::{self, AutoSession},
            Session, Signal as SessionSignal,
        },
    },
    nix::fcntl::OFlag,
    reexports::{gbm::Device as GbmDevice, wayland_server::Display},
    utils::signaling::{Linkable, Signaler},
};

use super::SessionFd;

#[derive(thiserror::Error, Debug)]
pub enum DeviceOpenError {
    #[error("Session error: {0}")]
    Session(#[from] auto::Error),
    #[error("Drm error: {0}")]
    Drm(#[from] DrmError),
    #[error("Gbm error: {0}")]
    Gbm(#[from] io::Error),
    #[error("Egl error: {0}")]
    Egl(#[from] egl::Error),
    #[error("Gles error: {0}")]
    Gles(#[from] Gles2Error),
}

pub struct Device<D> {
    pub drm: Dispatcher<'static, DrmDevice<SessionFd>, D>,
    pub gbm: Rc<RefCell<GbmDevice<SessionFd>>>,
    pub egl: EGLDisplay,
    pub renderer: Rc<RefCell<Gles2Renderer>>,
}

impl<D> Device<D> {
    /// Try to open the device
    pub fn open<F>(
        session: &mut AutoSession,
        session_signal: &Signaler<SessionSignal>,
        path: &Path,
        cb: F,
    ) -> Result<Self, DeviceOpenError>
    where
        F: FnMut(DrmEvent, &mut Option<DrmEventMetadata>, &mut D) + 'static,
    {
        let fd = session.open(
            path,
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
        )?;
        let fd = SessionFd(fd);

        let mut drm = DrmDevice::new(fd, true, None)?;

        drm.link(session_signal.clone());
        let drm = Dispatcher::new(drm, cb);

        let gbm = GbmDevice::new(fd)?;

        let egl = EGLDisplay::new(&gbm, None)?;

        let context = EGLContext::new(&egl, None)?;
        let renderer = unsafe { Gles2Renderer::new(context, None)? };

        let gbm = Rc::new(RefCell::new(gbm));
        let renderer = Rc::new(RefCell::new(renderer));

        Ok(Device {
            drm,
            gbm,
            egl,
            renderer,
        })
    }

    pub fn bind_wl_display(&mut self, display: &Display) {
        if self.renderer.borrow_mut().bind_wl_display(display).is_ok() {
            info!("EGL hardware-acceleration enabled");
        }
    }
}

//
// WIP
//

// pub struct DeviceMap<D> {
//     devices: IndexMap<dev_t, Device<D>>,
// }

// impl<D> DeviceMap<D> {
//     pub fn device_added(
//         &mut self,
//         session: &mut AutoSession,
//         session_signal: &Signaler<SessionSignal>,
//         device_id: dev_t,
//         path: &Path,
//     ) {
//         let device = Device::open(session, session_signal, &path, |_, _, _| {});
//     }

//     pub fn changed(&mut self, device_id: dev_t) {
//         //
//     }

//     pub fn removed(&mut self, device_id: dev_t) {
//         todo!("We don't support gpu hot swap");
//     }
// }
