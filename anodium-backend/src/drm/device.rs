use std::{
    cell::{Ref, RefMut},
    rc::Rc,
};

use indexmap::IndexMap;
use smithay::{
    backend::drm::{self, DrmDeviceFd},
    reexports::{
        calloop::{Dispatcher, LoopHandle},
        drm::control::{connector, crtc, Device as _},
    },
};

trait AsDrm {
    fn as_drm(&self) -> Ref<drm::DrmDevice>;
    fn as_drm_mut(&self) -> RefMut<drm::DrmDevice>;
}

impl<D> AsDrm for Dispatcher<'_, drm::DrmDevice, D> {
    fn as_drm(&self) -> Ref<drm::DrmDevice> {
        self.as_source_ref()
    }

    fn as_drm_mut(&self) -> RefMut<drm::DrmDevice> {
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
        device: DrmDeviceFd,
        mut cb: F,
    ) -> Result<Self, drm::DrmError>
    where
        F: FnMut(drm::DrmEvent, &mut Option<drm::DrmEventMetadata>, &mut D) + 'static,
        D: 'static,
    {
        let drm = drm::DrmDevice::new(device, true)?;

        let drm = Dispatcher::new(drm, move |event, meta, data: &mut D| cb(event, meta, data));
        let _registration_token = event_loop.register_dispatcher(drm.clone()).unwrap();

        Ok(Self {
            drm: Box::new(drm),
            connectors: Default::default(),
        })
    }

    pub fn inner(&self) -> Ref<drm::DrmDevice> {
        self.drm.as_drm()
    }

    pub fn inner_mut(&self) -> RefMut<drm::DrmDevice> {
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
            .filter_map(|conn| drm.get_connector(*conn, true).ok())
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
            .map(|conn| drm.get_connector(*conn, false).unwrap())
            .filter(|conn| conn.state() == connector::State::Connected)
            .inspect(|conn| info!("Connected: {:?}", conn.interface()));

        for connector in connector_info {
            let encoder_infos = connector
                .encoders()
                .iter()
                .flat_map(|encoder_handle| drm.get_encoder(*encoder_handle));

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
