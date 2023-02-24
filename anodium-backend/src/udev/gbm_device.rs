use std::collections::HashSet;

use smithay::{
    backend::{
        allocator::gbm::GbmAllocator,
        allocator::Format as DrmFormat,
        drm::{DrmDeviceFd, GbmBufferedSurface},
    },
    reexports::drm::control::{self, connector, crtc},
};

use super::drm_device::DrmDevice;

pub fn create_surface(
    device: &DrmDevice,
    allocator: GbmAllocator<DrmDeviceFd>,
    crtc: crtc::Handle,
    connector: connector::Handle,
    mode: control::Mode,
    formats: HashSet<DrmFormat>,
) -> GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()> {
    let drm_surface = device.create_surface(crtc, mode, &[connector]).unwrap();
    GbmBufferedSurface::new(drm_surface, allocator, formats).unwrap()
}
