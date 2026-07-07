use anyhow::Result;
use hidapi::{DeviceInfo, HidApi, HidDevice};

use crate::app_state::BatteryInfo;

pub mod a7_compatible;
pub mod g3;

pub trait BatteryProtocol: Send + Sync {
    fn name(&self) -> &'static str;
    fn matches(&self, info: &DeviceInfo) -> bool;
    fn read_battery(&self, device: &HidDevice) -> Result<BatteryInfo>;
}

pub fn all_protocols() -> Vec<Box<dyn BatteryProtocol>> {
    vec![
        Box::new(a7_compatible::A7CompatibleProtocol),
        Box::new(g3::G3Protocol),
    ]
}

pub fn try_read_with_any_protocol(
    api: &HidApi,
    info: &DeviceInfo,
) -> Option<(&'static str, BatteryInfo)> {
    for protocol in all_protocols() {
        if !protocol.matches(info) {
            continue;
        }

        let device = match info.open_device(api) {
            Ok(device) => device,
            Err(_) => continue,
        };

        match protocol.read_battery(&device) {
            Ok(battery) => return Some((protocol.name(), battery)),
            Err(_) => continue,
        }
    }

    None
}
