use std::collections::HashMap;

use smithay::reexports::drm::control::{connector, crtc, Device as ControlDevice};

pub trait CrtcMapper {
    fn map<'a>(
        &mut self,
        drm: &impl ControlDevice,
        connectors: impl Iterator<Item = &'a connector::Info> + Clone,
    );

    fn crtc_for_connector(&self, connector: &connector::Handle) -> Option<crtc::Handle>;
}

#[derive(Debug, Default)]
pub struct SimpleCrtcMapper {
    crtcs: HashMap<connector::Handle, crtc::Handle>,
}

impl SimpleCrtcMapper {
    pub fn new() -> Self {
        Self::default()
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

impl super::CrtcMapper for SimpleCrtcMapper {
    fn map<'a>(
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

    fn crtc_for_connector(&self, connector: &connector::Handle) -> Option<crtc::Handle> {
        self.crtcs.get(connector).copied()
    }
}
