use std::{
    collections::HashMap,
    iter::{Chain, Map},
};

use smithay::reexports::drm::control::{connector, crtc, Device as ControlDevice};

#[derive(Debug, Default)]
pub struct ScanResult {
    pub added: Vec<(connector::Info, Option<crtc::Handle>)>,
    pub removed: Vec<(connector::Info, Option<crtc::Handle>)>,
}

type ScanVecItem = (connector::Info, Option<crtc::Handle>);
type ScanVecIter = Map<std::vec::IntoIter<ScanVecItem>, Mapper>;
type Mapper = fn(ScanVecItem) -> ConnectorEvent;

impl IntoIterator for ScanResult {
    type Item = ConnectorEvent;
    type IntoIter = Chain<ScanVecIter, ScanVecIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.removed
            .into_iter()
            .map((|(connector, crtc)| ConnectorEvent::Disconnected { connector, crtc }) as Mapper)
            .chain(
                self.added.into_iter().map(
                    (|(connector, crtc)| ConnectorEvent::Connected { connector, crtc }) as Mapper,
                ),
            )
    }
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected {
        connector: connector::Info,
        crtc: Option<crtc::Handle>,
    },
    Disconnected {
        connector: connector::Info,
        crtc: Option<crtc::Handle>,
    },
}

#[derive(Debug, Default)]
pub struct ConnectorScanner {
    connectors: HashMap<connector::Handle, connector::Info>,
    crtcs: CrtcMapper,
}

impl ConnectorScanner {
    pub fn new() -> Self {
        Default::default()
    }

    /// Should be called on every device changed event
    pub fn scan_connectors(&mut self, drm: &impl ControlDevice) -> ScanResult {
        let res_handles = drm.resource_handles().unwrap();
        let connector_handles = res_handles.connectors();

        let mut added = Vec::new();
        let mut removed = Vec::new();

        for conn in connector_handles
            .iter()
            .filter_map(|conn| drm.get_connector(*conn, true).ok())
        {
            let curr_state = conn.state();

            use connector::State;
            if let Some(old) = self.connectors.insert(conn.handle(), conn.clone()) {
                match (old.state(), curr_state) {
                    (State::Connected, State::Disconnected) => removed.push(conn),
                    (State::Disconnected | State::Unknown, State::Connected) => added.push(conn),
                    //
                    (State::Connected, State::Connected) => {}
                    (State::Disconnected, State::Disconnected) => {}
                    //
                    (State::Unknown, _) => {}
                    (_, State::Unknown) => {}
                }
            } else if curr_state == State::Connected {
                added.push(conn)
            }
        }

        let removed = removed
            .into_iter()
            .map(|info| {
                let crtc = self.crtcs.for_connector(&info.handle());
                (info, crtc)
            })
            .collect();

        self.crtcs
            .scan_crtcs(drm, self.connectors.iter().map(|(_, info)| info));

        let added = added
            .into_iter()
            .map(|info| {
                let crtc = self.crtcs.for_connector(&info.handle());
                (info, crtc)
            })
            .collect();

        ScanResult { added, removed }
    }

    pub fn connectors(&self) -> &HashMap<connector::Handle, connector::Info> {
        &self.connectors
    }

    pub fn crtc_for_connector(&self, connector: &connector::Handle) -> Option<crtc::Handle> {
        self.crtcs.for_connector(connector)
    }
}

#[derive(Debug, Default)]
pub struct CrtcMapper {
    crtcs: HashMap<connector::Handle, crtc::Handle>,
}

impl CrtcMapper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Should be called on every device changed event
    pub fn scan_crtcs<'a>(
        &mut self,
        drm: &impl ControlDevice,
        connectors: impl Iterator<Item = &'a connector::Info> + Clone,
    ) {
        for connector in connectors
            .clone()
            .filter(|conn| conn.state() != connector::State::Connected)
        {
            self.crtcs.remove(&connector.handle());
        }

        let mut needs_crtc: Vec<&connector::Info> = connectors
            .filter(|conn| conn.state() == connector::State::Connected)
            .filter(|conn| !self.crtcs.contains_key(&conn.handle()))
            .collect();

        needs_crtc.retain(|connector| {
            if let Some(crtc) = self.restored_for_connector(drm, connector) {
                self.crtcs.insert(connector.handle(), crtc);

                // This connector no longer needs crtc so let's remove it
                false
            } else {
                true
            }
        });

        for connector in needs_crtc {
            if let Some(crtc) = self.pick_next_avalible_for_connector(drm, connector) {
                self.crtcs.insert(connector.handle(), crtc);
            }
        }
    }

    pub fn for_connector(&self, connector: &connector::Handle) -> Option<crtc::Handle> {
        self.crtcs.get(connector).copied()
    }

    fn is_taken(&self, crtc: &crtc::Handle) -> bool {
        self.crtcs.values().any(|v| v == crtc)
    }

    fn is_available(&self, crtc: &crtc::Handle) -> bool {
        !self.is_taken(crtc)
    }

    fn restored_for_connector(
        &self,
        drm: &impl ControlDevice,
        connector: &connector::Info,
    ) -> Option<crtc::Handle> {
        let encoder = connector.current_encoder()?;
        let encoder = drm.get_encoder(encoder).ok()?;
        let crtc = encoder.crtc()?;

        self.is_available(&crtc).then_some(crtc)
    }

    fn pick_next_avalible_for_connector(
        &self,
        drm: &impl ControlDevice,
        connector: &connector::Info,
    ) -> Option<crtc::Handle> {
        let res_handles = drm.resource_handles().ok()?;

        connector
            .encoders()
            .iter()
            .flat_map(|encoder_handle| drm.get_encoder(*encoder_handle))
            .find_map(|encoder_info| {
                res_handles
                    .filter_crtcs(encoder_info.possible_crtcs())
                    .into_iter()
                    .find(|crtc| self.is_available(crtc))
            })
    }
}
