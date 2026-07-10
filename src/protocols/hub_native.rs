use anyhow::{bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState, DeviceStatus};
use crate::protocols::BatteryProtocol;

/// 主 HUB 网页新型号协议（K7 / M7 / L7 / A5 Ultra / A5 V2 / A5 V3 / AX5 V2 / G3 Ultra / A7 V2+ 等）。
///
/// 来源：`www.mchose.com.cn` 打包 JS 中 `getBattery` 命令与 `ilt`/`Bbe` 解析器。
/// - 有线查询：`06 87 00 00 01 00 02 00`（feature report，report id 常为 0x06）
/// - 无线查询：`13 4a 01 00…`
/// - 主动上报：command=10 type=5 → batteryLevel + chargeStatus
pub struct HubNativeProtocol;

const VID_NEW: u16 = 0x3837;
const REPORT_LEN: usize = 65;

/// 厂商自定义 usage page（过滤掉普通鼠标/键盘集合）
const VENDOR_USAGE_PAGES: &[u16] = &[
    0xff00, 0xff01, 0xff0b, 0xff80, 0xfe00, 0xffa0, 0xffff,
];

impl BatteryProtocol for HubNativeProtocol {
    fn name(&self) -> &'static str {
        "Hub"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        let vid = info.vendor_id();
        // 旧 A5/AX5(0x2023)、旧 A7(0x5253) 有专属协议，不抢
        if vid == 0x2023 || vid == 0x5253 {
            return false;
        }

        let page = info.usage_page();
        let vendor_page =
            VENDOR_USAGE_PAGES.contains(&page) || page >= 0xff00;

        if vid == VID_NEW {
            return vendor_page || info.interface_number() >= 1;
        }

        // 其他 VID：名称像 MCHOSE 且厂商 usage page
        if !vendor_page {
            return false;
        }
        let product = info.product_string().unwrap_or("").to_ascii_lowercase();
        let manufacturer = info.manufacturer_string().unwrap_or("").to_ascii_lowercase();
        let text = format!("{manufacturer} {product}");
        text.contains("mchose") || text.contains("maicong") || text.contains("迈从")
    }

    fn read_status(&self, device: &HidDevice) -> Result<DeviceStatus> {
        if let Ok(status) = try_wired_feature(device) {
            return Ok(status);
        }
        if let Ok(status) = try_wireless_feature(device) {
            return Ok(status);
        }
        if let Ok(status) = try_input_report_poll(device) {
            return Ok(status);
        }
        bail!("hub native battery read failed on all variants");
    }
}

fn try_wired_feature(device: &HidDevice) -> Result<DeviceStatus> {
    // 网页：wiredCommand = 06 87 00 00 01 00 02 00 + padding
    let mut report = vec![0u8; REPORT_LEN];
    report[0] = 0x06;
    report[1] = 0x87;
    report[2] = 0x00;
    report[3] = 0x00;
    report[4] = 0x01;
    report[5] = 0x00;
    report[6] = 0x02;
    report[7] = 0x00;

    device.send_feature_report(&report)?;
    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut response = vec![0u8; REPORT_LEN];
    response[0] = 0x06;
    let len = device.get_feature_report(&mut response)?;
    parse_bbe_response(&response[..len], true)
        .or_else(|_| parse_ilt_from_buffer(&response[..len], true))
        .or_else(|_| try_read_input_after_cmd(device, true))
}

fn try_wireless_feature(device: &HidDevice) -> Result<DeviceStatus> {
    // 网页：wirelessCommand = 13 4a 01 00…
    let mut report = vec![0u8; REPORT_LEN];
    report[0] = 0x13;
    report[1] = 0x4a;
    report[2] = 0x01;

    // 优先 feature，失败再 write
    let sent = device
        .send_feature_report(&report)
        .or_else(|_| device.write(&report).map(|_| ()));
    sent?;
    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut response = vec![0u8; REPORT_LEN];
    response[0] = 0x13;
    if let Ok(len) = device.get_feature_report(&mut response) {
        if let Ok(status) = parse_bbe_response(&response[..len], false)
            .or_else(|_| parse_ilt_from_buffer(&response[..len], false))
        {
            return Ok(status);
        }
    }

    try_read_input_after_cmd(device, false)
}

fn try_input_report_poll(device: &HidDevice) -> Result<DeviceStatus> {
    // 不发送命令，尝试读缓存的 command=10 type=5 上报
    for _ in 0..4 {
        let mut buf = [0u8; REPORT_LEN];
        let len = device.read_timeout(&mut buf, 200)?;
        if len == 0 {
            continue;
        }
        if let Ok(status) = parse_ilt_from_buffer(&buf[..len], true)
            .or_else(|_| parse_ilt_from_buffer(&buf[..len], false))
        {
            return Ok(status);
        }
    }
    bail!("no hub battery input report");
}

fn try_read_input_after_cmd(device: &HidDevice, wired: bool) -> Result<DeviceStatus> {
    for _ in 0..8 {
        let mut buf = [0u8; REPORT_LEN];
        let len = device.read_timeout(&mut buf, 500)?;
        if len == 0 {
            continue;
        }
        if let Ok(status) = parse_ilt_from_buffer(&buf[..len], wired)
            .or_else(|_| parse_ilt_from_buffer(&buf[..len], !wired))
            .or_else(|_| parse_bbe_response(&buf[..len], wired))
        {
            return Ok(status);
        }
    }
    bail!("no hub battery response after command");
}

/// 网页 `Bbe(wired)`：wired 时前 6 字节 space0，再 batteryLevel + full:charge nibble
fn parse_bbe_response(buf: &[u8], wired: bool) -> Result<DeviceStatus> {
    let data = strip_report_id(buf);
    let base = if wired { 6 } else { 0 };
    if data.len() < base + 2 {
        bail!("bbe response too short");
    }
    let percent = data[base];
    let packed = data[base + 1];
    let full_status = packed & 0x0f;
    let charge_raw = packed >> 4;
    // 网页 bit4 fullStatus + bit4 chargeStatus；endianness little 时低 nibble 先
    // 上面按 packed 低 4 = full、高 4 = charge；若反了由 charge 映射兜底
    build_status(percent, charge_raw, full_status)
        .or_else(|_| build_status(percent, full_status, charge_raw))
}

/// 网页 `ilt(wired)`：command[, space0×3], type, batteryLevel, full:charge
/// 有线 n=true 无 space；无线 n=false 有 3 字节 space
fn parse_ilt_from_buffer(buf: &[u8], wired: bool) -> Result<DeviceStatus> {
    let data = strip_report_id(buf);
    if data.is_empty() {
        bail!("empty ilt buffer");
    }

    // 允许 report 内任意偏移找到 command=10 type=5
    let space = if wired { 0 } else { 3 };
    for start in 0..data.len().saturating_sub(3 + space) {
        if data[start] != 10 {
            continue;
        }
        let type_idx = start + 1 + space;
        if type_idx >= data.len() || data[type_idx] != 5 {
            continue;
        }
        let bat_idx = type_idx + 1;
        let pack_idx = type_idx + 2;
        if pack_idx >= data.len() {
            continue;
        }
        let percent = data[bat_idx];
        let packed = data[pack_idx];
        let full_status = packed & 0x0f;
        let charge_raw = packed >> 4;
        if let Ok(status) = build_status(percent, charge_raw, full_status)
            .or_else(|_| build_status(percent, full_status, charge_raw))
        {
            return Ok(status);
        }
    }

    bail!("ilt command=10 type=5 not found");
}

fn strip_report_id(buf: &[u8]) -> &[u8] {
    // 若首字节像 report id（常见 0x06/0x0a/0x13/0x00），且后面是 command 结构，保留整段由解析器扫
    buf
}

fn build_status(percent: u8, charge_raw: u8, full_status: u8) -> Result<DeviceStatus> {
    if percent > 100 {
        bail!("invalid hub battery percent: {percent}");
    }

    // fullStatus 非 0 时常表示充满（网页 fullStatus 会把显示电量置 100）
    let charge_state = if full_status != 0 && percent >= 95 {
        ChargeState::Full
    } else {
        match charge_raw {
            0 => ChargeState::NotCharging,
            1 => ChargeState::Charging,
            2 => ChargeState::Full,
            _ if charge_raw != 0 => ChargeState::Charging,
            _ => ChargeState::Unknown,
        }
    };

    Ok(DeviceStatus {
        battery: BatteryInfo {
            percent,
            charge_state,
        },
        dpi: None,
    })
}
