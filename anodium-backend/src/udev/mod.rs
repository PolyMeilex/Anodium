pub mod drm_scanner;
pub mod edid;
mod hwdata;

use std::path::PathBuf;

use smithay::{
    backend::{
        drm::{DrmNode, NodeType},
        udev,
    },
    output::Mode as WlMode,
    reexports::drm::control::{connector, Mode as DrmMode, ModeFlags},
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

pub fn format_connector_name(connector_info: &connector::Info) -> String {
    let interface_id = connector_info.interface_id();

    // TODO: Remove once supported in drm-rs
    use connector::Interface;
    let interface_short_name = match connector_info.interface() {
        Interface::Unknown => "Unknown",
        Interface::VGA => "VGA",
        Interface::DVII => "DVI-I",
        Interface::DVID => "DVI-D",
        Interface::DVIA => "DVI-A",
        Interface::Composite => "Composite",
        Interface::SVideo => "SVIDEO",
        Interface::LVDS => "LVDS",
        Interface::Component => "Component",
        Interface::NinePinDIN => "DIN",
        Interface::DisplayPort => "DP",
        Interface::HDMIA => "HDMI-A",
        Interface::HDMIB => "HDMI-B",
        Interface::TV => "TV",
        Interface::EmbeddedDisplayPort => "eDP",
        Interface::Virtual => "Virtual",
        Interface::DSI => "DSI",
        Interface::DPI => "DPI",
    };

    format!("{interface_short_name}-{interface_id}")
}

pub fn drm_mode_to_wl_mode(mode: DrmMode) -> WlMode {
    let clock = mode.clock() as u64;
    let htotal = mode.hsync().2 as u64;
    let vtotal = mode.vsync().2 as u64;

    let mut refresh = (clock * 1_000_000 / htotal + vtotal / 2) / vtotal;

    if mode.flags().contains(ModeFlags::INTERLACE) {
        refresh *= 2;
    }

    if mode.flags().contains(ModeFlags::DBLSCAN) {
        refresh /= 2;
    }

    if mode.vscan() > 1 {
        refresh /= mode.vscan() as u64;
    }

    let (w, h) = mode.size();

    WlMode {
        size: (w as i32, h as i32).into(),
        refresh: refresh as i32,
    }
}
