pub mod drm_device;
pub mod drm_scanner;

use std::path::PathBuf;

use smithay::backend::{
    drm::{DrmNode, NodeType},
    udev,
};

pub fn primary_gpu(seat: &str) -> (DrmNode, PathBuf) {
    udev::primary_gpu(seat)
        .unwrap()
        .and_then(|p| {
            DrmNode::from_path(&p)
                .ok()?
                .node_with_type(NodeType::Render)?
                .ok()
                .map(|node| (node, p))
        })
        .unwrap_or_else(|| {
            udev::all_gpus(seat)
                .unwrap()
                .into_iter()
                .find_map(|p| DrmNode::from_path(&p).ok().map(|node| (node, p)))
                .expect("No GPU!")
        })
}
