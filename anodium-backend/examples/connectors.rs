use std::{fs::OpenOptions, os::unix::prelude::OwnedFd, time::Duration};

use smithay::{
    backend::drm::{DrmDevice, DrmDeviceFd},
    reexports::calloop::{timer::Timer, EventLoop},
    reexports::drm::control::Device as ControlDevice,
    utils::DeviceFd,
};

fn main() {
    /*
     * Initialize the drm backend
     */

    // "Find" a suitable drm device
    let mut options = OpenOptions::new();
    options.read(true);
    options.write(true);

    let fd = DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(
        options.open("/dev/dri/card0").unwrap(),
    )));

    let (device, device_notifier) = DrmDevice::new(fd, false).unwrap();

    // Get a set of all modesetting resource handles (excluding planes):
    let res_handles = device.resource_handles().unwrap();

    // Use first connected connector
    for connector_info in res_handles
        .connectors()
        .iter()
        .map(|conn| device.get_connector(*conn, false).unwrap())
    {
        dbg!(connector_info.current_encoder());
    }

    // /*
    //  * Register the DrmDevice on the EventLoop
    //  */
    let mut event_loop = EventLoop::<()>::try_new().unwrap();

    event_loop
        .handle()
        .insert_source(device_notifier, move |event, _: &mut _, _: &mut ()| {
            dbg!(event);
        })
        .unwrap();

    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(5)), |_, _, _| {
            std::process::exit(0);
        })
        .unwrap();

    // Run
    event_loop.run(None, &mut (), |_| {}).unwrap();
}
