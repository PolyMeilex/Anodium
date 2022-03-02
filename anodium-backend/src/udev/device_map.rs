use std::{cell::RefCell, io, path::Path, rc::Rc};

use calloop::Dispatcher;
use indexmap::IndexMap;
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
    reexports::{
        drm::control::{connector, crtc, Device as _},
        gbm::Device as GbmDevice,
        wayland_server::Display,
    },
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
    pub connectors: IndexMap<connector::Handle, connector::Info>,
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
            connectors: IndexMap::new(),
        })
    }

    pub fn bind_wl_display(&mut self, display: &Display) {
        if self.renderer.borrow_mut().bind_wl_display(display).is_ok() {
            info!("EGL hardware-acceleration enabled");
        }
    }

    pub fn scan_connectors(&mut self) -> ScanResult {
        let drm = self.drm.as_source_ref();
        // Get a set of all modesetting resource handles (excluding planes):
        let res_handles = drm.resource_handles().unwrap();
        let connector_handles = res_handles.connectors();

        let mut added = Vec::new();
        let mut removed = Vec::new();

        for conn in connector_handles
            .iter()
            .filter_map(|conn| drm.get_connector(*conn).ok())
        {
            let handle = conn.handle();
            let curr_state = conn.state();

            let old = self.connectors.insert(handle, conn);

            if let Some(old) = old {
                use connector::State;
                match (old.state(), curr_state) {
                    (State::Connected, State::Disconnected) => removed.push(handle),
                    (State::Disconnected, State::Connected) => added.push(handle),
                    //
                    (State::Connected, State::Connected) => {}
                    (State::Disconnected, State::Disconnected) => {}
                    //
                    (State::Unknown, _) => todo!(),
                    (_, State::Unknown) => todo!(),
                }
            } else {
                added.push(handle)
            }
        }

        let mut connectors = IndexMap::new();

        let connector_info = connector_handles
            .iter()
            .map(|conn| drm.get_connector(*conn).unwrap())
            .filter(|conn| conn.state() == connector::State::Connected)
            .inspect(|conn| info!("Connected: {:?}", conn.interface()));

        for connector in connector_info {
            let encoder_infos = connector
                .encoders()
                .iter()
                .filter_map(|e| *e)
                .flat_map(|encoder_handle| drm.get_encoder(encoder_handle));

            'outer: for encoder_info in encoder_infos {
                for crtc in res_handles.filter_crtcs(encoder_info.possible_crtcs()) {
                    if !connectors.values().any(|v| *v == crtc) {
                        connectors.insert(connector.handle(), crtc);
                        break 'outer;
                    }
                }
            }
        }

        ScanResult {
            map: connectors,
            added,
            removed,
        }
    }
}

pub struct ScanResult {
    pub map: IndexMap<connector::Handle, crtc::Handle>,
    pub added: Vec<connector::Handle>,
    pub removed: Vec<connector::Handle>,
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
