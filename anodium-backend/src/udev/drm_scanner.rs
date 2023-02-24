use std::{
    collections::HashMap,
    iter::{Chain, Map},
};

use indexmap::IndexMap;
use smithay::{
    backend::drm,
    reexports::drm::control::{connector, crtc, Device as _},
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

#[derive(Debug, Default)]
pub struct ConnectorScanner {
    connectors: IndexMap<connector::Handle, connector::Info>,
}

impl ConnectorScanner {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn scan_connectors(&mut self, drm: &drm::DrmDevice) -> ScanResult {
        let res_handles = drm.resource_handles().unwrap();
        let connector_handles = res_handles.connectors();

        let mut added = Vec::new();
        let mut removed = Vec::new();

        for conn in connector_handles
            .iter()
            .filter_map(|conn| drm.get_connector(*conn, true).ok())
        {
            let curr_state = conn.state();

            let old = self.connectors.insert(conn.handle(), conn.clone());

            use connector::State;
            if let Some(old) = old {
                match (old.state(), curr_state) {
                    (State::Connected, State::Disconnected) => removed.push(conn),
                    (State::Disconnected, State::Connected) => added.push(conn),
                    //
                    (State::Connected, State::Connected) => {}
                    (State::Disconnected, State::Disconnected) => {}
                    //
                    (State::Unknown, _) => todo!(),
                    (_, State::Unknown) => todo!(),
                }
            } else if curr_state == State::Connected {
                added.push(conn)
            }
        }

        ScanResult { added, removed }
    }

    pub fn connectors(&self) -> &IndexMap<connector::Handle, connector::Info> {
        &self.connectors
    }
}

#[derive(Debug, Default)]
pub struct CrtcsScanner {
    crtcs: HashMap<connector::Info, crtc::Handle>,
}

impl CrtcsScanner {
    pub fn scan_crtcs(&mut self, drm: &drm::DrmDevice) -> HashMap<connector::Info, crtc::Handle> {
        let resource_handles = drm.resource_handles().unwrap();

        let connector_info = resource_handles
            .connectors()
            .iter()
            .map(|conn| drm.get_connector(*conn, false).unwrap())
            .filter(|conn| conn.state() == connector::State::Connected);

        for connector in connector_info {
            self.for_connector(drm, connector);
        }

        self.crtcs.clone()
    }

    pub fn for_connector(
        &mut self,
        drm: &drm::DrmDevice,
        connector: connector::Info,
    ) -> Option<crtc::Handle> {
        if let Some(crtc) = self.crtcs.get(&connector) {
            return Some(*crtc);
        }

        let res_handles = drm.resource_handles().unwrap();

        let encoder_infos = connector
            .encoders()
            .iter()
            .flat_map(|encoder_handle| drm.get_encoder(*encoder_handle));

        for encoder_info in encoder_infos {
            for crtc in res_handles.filter_crtcs(encoder_info.possible_crtcs()) {
                if !self.crtcs.values().any(|v| *v == crtc) {
                    self.crtcs.insert(connector, crtc);
                    return Some(crtc);
                }
            }
        }

        None
    }

    pub fn remove_connector(&mut self, connector: &connector::Info) -> Option<crtc::Handle> {
        self.crtcs.remove(connector)
    }
}
