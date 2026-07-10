use anyhow::Result;
use hidapi::{DeviceInfo, HidApi, HidDevice};

use crate::app_state::DeviceStatus;

pub mod a5_pro;
pub mod a7_compatible;
pub mod g3;
pub mod hub_native;

pub trait BatteryProtocol: Send + Sync {
    fn name(&self) -> &'static str;
    fn matches(&self, info: &DeviceInfo) -> bool;
    fn read_status(&self, device: &HidDevice) -> Result<DeviceStatus>;
}

pub fn all_protocols() -> Vec<Box<dyn BatteryProtocol>> {
    vec![
        // 精确匹配优先
        Box::new(a5_pro::A5ProProtocol),
        Box::new(a7_compatible::A7CompatibleProtocol),
        // gmouse 网页族（usage 0xFF01）
        Box::new(g3::GmouseProtocol),
        // 主 HUB 新型号（较宽匹配，放后）
        Box::new(hub_native::HubNativeProtocol),
    ]
}

pub fn try_read_with_any_protocol(
    api: &HidApi,
    info: &DeviceInfo,
) -> Option<(&'static str, DeviceStatus)> {
    for protocol in all_protocols() {
        if !protocol.matches(info) {
            continue;
        }

        let device = match info.open_device(api) {
            Ok(device) => device,
            Err(_) => continue,
        };

        match protocol.read_status(&device) {
            Ok(status) => return Some((protocol.name(), status)),
            Err(_) => continue,
        }
    }

    None
}
