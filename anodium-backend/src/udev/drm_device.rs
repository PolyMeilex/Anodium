use std::{os::unix::prelude::FromRawFd, path::Path};

use smithay::{
    backend::{
        drm::{self, DrmDeviceFd},
        session::Session,
    },
    reexports::nix::fcntl::OFlag,
    utils::DeviceFd,
};

pub fn new(session: &mut impl Session, path: &Path) -> (drm::DrmDevice, drm::DrmDeviceNotifier) {
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
