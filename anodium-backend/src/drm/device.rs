use std::{
    cell::{Ref, RefMut},
    os::unix::prelude::{AsRawFd, RawFd},
    path::Path,
    sync::Arc,
};

use indexmap::IndexMap;
use smithay::{
    backend::{
        drm,
        session::{
            auto::{AutoSession, Error as SessionError},
            Session,
        },
    },
    nix::{fcntl::OFlag, unistd},
    reexports::{
        calloop::{Dispatcher, LoopHandle},
        drm::control::{connector, crtc, Device as _},
    },
};

#[derive(Debug)]
struct Inner {
    fd: RawFd,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unistd::close(self.fd).ok();
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    inner: Arc<Inner>,
}

impl Device {
    /// Try to open the device
    pub fn open(session: &mut AutoSession, path: &Path) -> Result<Self, SessionError> {
        let fd = session.open(
            path,
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
        )?;

        Ok(Device {
            inner: Arc::new(Inner { fd }),
        })
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.fd
    }
}

trait AsDrm {
    fn as_drm(&self) -> Ref<drm::DrmDevice<Device>>;
    fn as_drm_mut(&self) -> RefMut<drm::DrmDevice<Device>>;
}

impl<D> AsDrm for Dispatcher<'_, drm::DrmDevice<Device>, D> {
    fn as_drm(&self) -> Ref<drm::DrmDevice<Device>> {
        self.as_source_ref()
    }

    fn as_drm_mut(&self) -> RefMut<drm::DrmDevice<Device>> {
        self.as_source_mut()
    }
}

pub struct DrmDevice {
    drm: Box<dyn AsDrm>,
    connectors: IndexMap<connector::Handle, connector::Info>,
}

impl DrmDevice {
    pub fn new<D, F>(
        event_loop: &LoopHandle<'static, D>,
        device: Device,
        mut cb: F,
    ) -> Result<Self, drm::DrmError>
    where
        F: FnMut(drm::DrmEvent, &mut Option<drm::DrmEventMetadata>, &mut D) + 'static,
        D: 'static,
    {
        let drm = drm::DrmDevice::new(device, true, None)?;

        let drm = Dispatcher::new(drm, move |event, meta, data: &mut D| cb(event, meta, data));
        let _registration_token = event_loop.register_dispatcher(drm.clone()).unwrap();

        Ok(Self {
            drm: Box::new(drm),
            connectors: Default::default(),
        })
    }

    pub fn inner(&self) -> Ref<drm::DrmDevice<Device>> {
        self.drm.as_drm()
    }

    pub fn inner_mut(&self) -> RefMut<drm::DrmDevice<Device>> {
        self.drm.as_drm_mut()
    }

    pub fn scan_connectors(&mut self) -> ScanResult {
        let drm = self.drm.as_drm();
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

#[derive(Debug)]
pub struct ScanResult {
    pub map: IndexMap<connector::Handle, crtc::Handle>,
    pub added: Vec<connector::Handle>,
    pub removed: Vec<connector::Handle>,
}
