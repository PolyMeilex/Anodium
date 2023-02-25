use smithay::reexports::drm::control::{
    connector, property, Device as ControlDevice, PropertyValueSet,
};

pub struct EdidInfo {
    pub model: String,
    pub manufacturer: String,
}

impl EdidInfo {
    pub fn for_connector(
        device: &impl ControlDevice,
        connector: connector::Handle,
    ) -> Option<EdidInfo> {
        device
            .get_properties(connector)
            .ok()
            .and_then(|props| get_edid(device, &props))
            .map(|edid| EdidInfo {
                model: get_monitor_name(&edid),
                manufacturer: get_manufacturer_name(&edid),
            })
    }
}

/// Returns iterator over pairs representing a set of [`property::Handle`] and their raw values
fn iter(
    props: &PropertyValueSet,
) -> impl Iterator<Item = (&property::Handle, &property::RawValue)> {
    let (ids, values) = props.as_props_and_values();
    ids.iter().zip(values.iter())
}

fn get_edid(device: &impl ControlDevice, props: &PropertyValueSet) -> Option<edid_rs::EDID> {
    iter(props)
        .filter_map(|(&handle, value)| {
            let info = device.get_property(handle).ok()?;

            Some((info, value))
        })
        .find(|(info, _)| info.name().to_str() == Ok("EDID"))
        .and_then(|(info, &value)| {
            if let property::Value::Blob(edid_blob) = info.value_type().convert_value(value) {
                Some(edid_blob)
            } else {
                None
            }
        })
        .and_then(|blob| {
            let data = device.get_property_blob(blob).ok()?;
            let mut reader = std::io::Cursor::new(data);

            edid_rs::parse(&mut reader).ok()
        })
}

fn get_manufacturer_name(edid: &edid_rs::EDID) -> String {
    let id = edid.product.manufacturer_id;
    let code = [id.0, id.1, id.2];

    hwdata::find_manufacturer(&code)
        .map(|name| name.to_string())
        .unwrap_or_else(|| code.into_iter().collect())
}

fn get_monitor_name(edid: &edid_rs::EDID) -> String {
    edid.descriptors
        .0
        .iter()
        .find_map(|desc| {
            if let edid_rs::MonitorDescriptor::MonitorName(name) = desc {
                Some(name.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| edid.product.product_code.to_string())
}

mod hwdata {
    include!(concat!(env!("OUT_DIR"), "/hwdata_pnp_ids.rs",));
}
