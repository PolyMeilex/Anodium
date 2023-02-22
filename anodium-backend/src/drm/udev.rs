use std::path::PathBuf;

use anyhow::Result;
use smithay::{
    backend::{
        drm::{DrmNode, NodeType},
        udev::{self, UdevBackend, UdevEvent},
    },
    reexports::calloop::LoopHandle,
};

use super::gpu::Gpu;
use crate::BackendHandler;

pub fn primary_gpu(seat: &str) -> (PathBuf, DrmNode) {
    udev::primary_gpu(seat)
        .unwrap()
        .and_then(|p| {
            DrmNode::from_path(&p)
                .ok()?
                .node_with_type(NodeType::Render)?
                .ok()
                .map(|node| (p, node))
        })
        .unwrap_or_else(|| {
            udev::all_gpus(seat)
                .unwrap()
                .into_iter()
                .find_map(|p| DrmNode::from_path(&p).ok().map(|node| (p, node)))
                .expect("No GPU!")
        })
}

pub fn init<D>(event_loop: LoopHandle<D>, seat: String) -> Result<()>
where
    D: BackendHandler,
{
    let udev_backend = UdevBackend::new(seat)?;

    event_loop
        .insert_source(udev_backend, move |event, _, handler| match event {
            UdevEvent::Added { .. } => {
                error!("GPU hotplug not supported");
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(drm_node) = DrmNode::from_dev_id(device_id) {
                    Gpu::changed_event(drm_node, handler);
                }
            }
            UdevEvent::Removed { .. } => {
                error!("GPU hotplug not supported");
            }
        })
        .unwrap();

    Ok(())
}
