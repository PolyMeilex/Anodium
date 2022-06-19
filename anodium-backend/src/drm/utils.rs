use smithay::reexports::drm::control::connector;

pub fn format_connector_name(interface: connector::Interface, interface_id: u32) -> String {
    let other_short_name;
    let interface_short_name = match interface {
        connector::Interface::DVII => "DVI-I",
        connector::Interface::DVID => "DVI-D",
        connector::Interface::DVIA => "DVI-A",
        connector::Interface::SVideo => "S-VIDEO",
        connector::Interface::DisplayPort => "DP",
        connector::Interface::HDMIA => "HDMI-A",
        connector::Interface::HDMIB => "HDMI-B",
        connector::Interface::EmbeddedDisplayPort => "eDP",
        other => {
            other_short_name = format!("{:?}", other);
            &other_short_name
        }
    };

    format!("{}-{}", interface_short_name, interface_id)
}
