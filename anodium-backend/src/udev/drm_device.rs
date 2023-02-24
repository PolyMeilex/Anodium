use std::{
    cell::{Ref, RefCell},
    iter::{Chain, Map},
    os::unix::prelude::FromRawFd,
    path::Path,
    rc::Rc,
};

use indexmap::IndexMap;
use smithay::{
    backend::{
        drm::{self, DrmDeviceFd, DrmNode, DrmSurface},
        session::Session,
    },
    reexports::{
        calloop::EventSource,
        drm::{
            control::{self, connector, crtc, Device as _},
            SystemError,
        },
        nix::fcntl::OFlag,
    },
    utils::DeviceFd,
};

#[derive(Debug, Default)]
pub struct ScanResult {
    pub added: Vec<connector::Info>,
    pub removed: Vec<connector::Info>,
}

type Mapper = fn(connector::Info) -> ConnectorEvent;

impl IntoIterator for ScanResult {
    type Item = ConnectorEvent;
    type IntoIter = Chain<
        Map<std::vec::IntoIter<connector::Info>, Mapper>,
        Map<std::vec::IntoIter<connector::Info>, Mapper>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.removed
            .into_iter()
            .map(ConnectorEvent::Disconnected as Mapper)
            .chain(
                self.added
                    .into_iter()
                    .map(ConnectorEvent::Connected as Mapper),
            )
    }
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected(connector::Info),
    Disconnected(connector::Info),
}

#[derive(Debug)]
pub struct Inner {
    drm: drm::DrmDevice,
    fd: DrmDeviceFd,
}

impl Inner {
    pub fn new(fd: DrmDeviceFd) -> Self {
        let drm = drm::DrmDevice::new(fd.clone(), true).unwrap();

        Self { drm, fd }
    }
}

#[derive(Debug, Clone)]
pub struct DrmDevice {
    inner: Rc<RefCell<Inner>>,
    node: DrmNode,
}

impl DrmDevice {
    pub fn new(session: &mut impl Session, node: DrmNode, path: &Path) -> Self {
        let fd = session
            .open(
                path,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            )
            .unwrap();

        let fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd) });

        Self {
            inner: Rc::new(RefCell::new(Inner::new(fd))),
            node,
        }
    }

    pub fn fd(&self) -> DrmDeviceFd {
        self.inner.borrow().fd.clone()
    }

    pub fn get_connector(
        &self,
        handle: connector::Handle,
        force_probe: bool,
    ) -> Result<connector::Info, SystemError> {
        self.inner.borrow().drm.get_connector(handle, force_probe)
    }

    pub fn create_surface(
        &self,
        crtc: crtc::Handle,
        mode: control::Mode,
        connectors: &[connector::Handle],
    ) -> Result<DrmSurface, drm::DrmError> {
        self.inner
            .borrow()
            .drm
            .create_surface(crtc, mode, connectors)
    }

    pub fn node(&self) -> DrmNode {
        self.node
    }

    pub fn borrow(&self) -> Ref<drm::DrmDevice> {
        Ref::map(self.inner.borrow(), |i| &i.drm)
    }
}

impl EventSource for DrmDevice {
    type Event = drm::DrmEvent;
    type Metadata = Option<drm::DrmEventMetadata>;
    type Ret = ();
    type Error = std::io::Error;

    fn process_events<F>(
        &mut self,
        readiness: smithay::reexports::calloop::Readiness,
        token: smithay::reexports::calloop::Token,
        callback: F,
    ) -> Result<smithay::reexports::calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.inner
            .borrow_mut()
            .drm
            .process_events(readiness, token, callback)
    }

    fn register(
        &mut self,
        poll: &mut smithay::reexports::calloop::Poll,
        token_factory: &mut smithay::reexports::calloop::TokenFactory,
    ) -> smithay::reexports::calloop::Result<()> {
        self.inner.borrow_mut().drm.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut smithay::reexports::calloop::Poll,
        token_factory: &mut smithay::reexports::calloop::TokenFactory,
    ) -> smithay::reexports::calloop::Result<()> {
        self.inner.borrow_mut().drm.reregister(poll, token_factory)
    }

    fn unregister(
        &mut self,
        poll: &mut smithay::reexports::calloop::Poll,
    ) -> smithay::reexports::calloop::Result<()> {
        self.inner.borrow_mut().drm.unregister(poll)
    }
}
