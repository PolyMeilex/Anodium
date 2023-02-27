use std::iter::{Chain, Map};

use smithay::reexports::drm::control::{connector, crtc, Device as ControlDevice};

mod connector_scanner;
pub use connector_scanner::{ConnectorScanEvent, ConnectorScanResult, ConnectorScanner};

mod crtc_mapper;
pub use crtc_mapper::{CrtcMapper, SimpleCrtcMapper};

#[derive(Debug, Default)]
pub struct DrmScanner<Mapper = SimpleCrtcMapper>
where
    Mapper: CrtcMapper,
{
    connectors: ConnectorScanner,
    crtc_mapper: Mapper,
}

impl<M> DrmScanner<M>
where
    M: CrtcMapper + Default,
{
    pub fn new() -> Self {
        Self::new_with_mapper(Default::default())
    }
}

impl<M> DrmScanner<M>
where
    M: CrtcMapper,
{
    pub fn new_with_mapper(mapper: M) -> Self {
        Self {
            connectors: Default::default(),
            crtc_mapper: mapper,
        }
    }

    pub fn crtc_mapper(&self) -> &M {
        &self.crtc_mapper
    }

    pub fn crtc_mapper_mut(&mut self) -> &mut M {
        &mut self.crtc_mapper
    }

    /// Should be called on every device changed event
    pub fn scan_connectors(&mut self, drm: &impl ControlDevice) -> DrmScanResult {
        let scan = self.connectors.scan(drm);

        let removed = scan
            .removed
            .into_iter()
            .map(|info| {
                let crtc = self.crtc_mapper.crtc_for_connector(&info.handle());
                (info, crtc)
            })
            .collect();

        self.crtc_mapper.map(
            drm,
            self.connectors.connectors().iter().map(|(_, info)| info),
        );

        let added = scan
            .added
            .into_iter()
            .map(|info| {
                let crtc = self.crtc_mapper.crtc_for_connector(&info.handle());
                (info, crtc)
            })
            .collect();

        DrmScanResult { removed, added }
    }

    pub fn crtc_for_connector(&self, connector: &connector::Handle) -> Option<crtc::Handle> {
        self.crtc_mapper.crtc_for_connector(connector)
    }
}

type DrmScanItem = (connector::Info, Option<crtc::Handle>);

#[derive(Debug, Default, Clone)]
pub struct DrmScanResult {
    pub added: Vec<DrmScanItem>,
    pub removed: Vec<DrmScanItem>,
}

impl DrmScanResult {
    pub fn iter(&self) -> impl Iterator<Item = DrmScanEvent> {
        self.clone().into_iter()
    }
}

#[derive(Debug, Clone)]
pub enum DrmScanEvent {
    Connected {
        connector: connector::Info,
        crtc: Option<crtc::Handle>,
    },
    Disconnected {
        connector: connector::Info,
        crtc: Option<crtc::Handle>,
    },
}

impl DrmScanEvent {
    fn connected((connector, crtc): (connector::Info, Option<crtc::Handle>)) -> Self {
        DrmScanEvent::Connected { connector, crtc }
    }

    fn disconnected((connector, crtc): (connector::Info, Option<crtc::Handle>)) -> Self {
        DrmScanEvent::Connected { connector, crtc }
    }
}

type DrmScanItemToEvent = fn(DrmScanItem) -> DrmScanEvent;

impl IntoIterator for DrmScanResult {
    type Item = DrmScanEvent;
    type IntoIter = Chain<
        Map<std::vec::IntoIter<DrmScanItem>, DrmScanItemToEvent>,
        Map<std::vec::IntoIter<DrmScanItem>, DrmScanItemToEvent>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.removed
            .into_iter()
            .map(DrmScanEvent::disconnected as DrmScanItemToEvent)
            .chain(
                self.added
                    .into_iter()
                    .map(DrmScanEvent::connected as DrmScanItemToEvent),
            )
    }
}
