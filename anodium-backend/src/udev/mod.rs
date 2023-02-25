pub mod drm_scanner;
pub mod edid;

use std::path::PathBuf;

use smithay::{
    backend::{
        drm::{DrmNode, NodeType},
        udev,
    },
    reexports::drm::control::connector,
};

use std::{os::unix::prelude::FromRawFd, path::Path};

use smithay::{
    backend::{
        drm::{self, DrmDeviceFd},
        session::Session,
    },
    reexports::nix::fcntl::OFlag,
    utils::DeviceFd,
};

pub fn new_drm_device(
    session: &mut impl Session,
    path: &Path,
) -> (drm::DrmDevice, drm::DrmDeviceNotifier) {
    let fd = session
        .open(
            path,
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
        )
        .unwrap();

    let fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd) });

    let (drm, drm_notifier) = drm::DrmDevice::new(fd, false).unwrap();

    (drm, drm_notifier)
}

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

pub fn format_connector_name(connector_info: &connector::Info) -> String {
    let interface_id = connector_info.interface_id();

    let tmp_short_name;
    let interface_short_name = match connector_info.interface() {
        connector::Interface::DVII => "DVI-I",
        connector::Interface::DVID => "DVI-D",
        connector::Interface::DVIA => "DVI-A",
        connector::Interface::SVideo => "S-VIDEO",
        connector::Interface::DisplayPort => "DP",
        connector::Interface::HDMIA => "HDMI-A",
        connector::Interface::HDMIB => "HDMI-B",
        connector::Interface::EmbeddedDisplayPort => "eDP",
        other => {
            tmp_short_name = format!("{other:?}");
            &tmp_short_name
        }
    };

    format!("{interface_short_name}-{interface_id}")
}
