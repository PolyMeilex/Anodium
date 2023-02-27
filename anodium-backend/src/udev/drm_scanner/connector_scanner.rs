use std::{
    collections::HashMap,
    iter::{Chain, Map},
};

use smithay::reexports::drm::control::{connector, Device as ControlDevice};

#[derive(Debug, Default)]
pub struct ConnectorScanner {
    connectors: HashMap<connector::Handle, connector::Info>,
}

impl ConnectorScanner {
    pub fn new() -> Self {
        Default::default()
    }

    /// Should be called on every device changed event
    pub fn scan(&mut self, drm: &impl ControlDevice) -> ConnectorScanResult {
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

        ConnectorScanResult { added, removed }
    }

    pub fn connectors(&self) -> &HashMap<connector::Handle, connector::Info> {
        &self.connectors
    }
}

#[derive(Debug, Default, Clone)]
pub struct ConnectorScanResult {
    pub added: Vec<connector::Info>,
    pub removed: Vec<connector::Info>,
}

#[derive(Debug, Clone)]
pub enum ConnectorScanEvent {
    Connected(connector::Info),
    Disconnected(connector::Info),
}

impl ConnectorScanResult {
    pub fn iter(&self) -> impl Iterator<Item = ConnectorScanEvent> {
        self.clone().into_iter()
    }
}

type ConnectorScanItemToEvent = fn(connector::Info) -> ConnectorScanEvent;

impl IntoIterator for ConnectorScanResult {
    type Item = ConnectorScanEvent;
    type IntoIter = Chain<
        Map<std::vec::IntoIter<connector::Info>, ConnectorScanItemToEvent>,
        Map<std::vec::IntoIter<connector::Info>, ConnectorScanItemToEvent>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.removed
            .into_iter()
            .map(ConnectorScanEvent::Disconnected as ConnectorScanItemToEvent)
            .chain(
                self.added
                    .into_iter()
                    .map(ConnectorScanEvent::Connected as ConnectorScanItemToEvent),
            )
    }
}
