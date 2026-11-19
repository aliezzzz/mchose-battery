use anyhow::{bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState, DeviceStatus, DpiInfo};
use crate::protocols::BatteryProtocol;

/// MCHOSE 新一代 Nordic 鼠标协议（K5 / K5 Pro / K5 Ultra 典藏版、A5 V3、A7 V3、K7 V2 等）。
///
/// 来源：`www.mchose.com.cn` 主 HUB 的 `newMouse:cmdStruct`。
/// 报文固定 64 字节，首字节是 HID report id `0x4D`：
/// `4d 01 01 <len> <cmd lo> <cmd hi> 00 00 <data…> <xor> …`
/// 命令：`0x0900` 设备信息、`0x0002` 当前配置档、`0x0003` DPI。
/// 响应 payload 的 body 从偏移 7 开始。
pub struct McNewMouseProtocol;

const VID: u16 = 0x3837;
const REPORT_ID: u8 = 0x4D;
const REPORT_LEN: usize = 64;
const CMD_INFO: u16 = 0x0900;
const CMD_ALL_INFO: u16 = 0x0002;
const CMD_DPI: u16 = 0x0003;
const BODY_OFFSET: usize = 7;

/// K5 家族 PID（十进制来自 HUB `mouseDiffConfig`）。
const PIDS: &[u16] = &[
    0x4038, // 16440 K5 Ultra 有线
    0x4037, // 16439 K5 Pro 有线
    0x4040, // 16448 K5 V2 有线
    0x1014, // 4116 新一代 2.4G 接收器
    0x1018, // 4120 新一代 2.4G 接收器（K5 Ultra 典藏版实测）
    0x101c, // 4124 新一代 2.4G 接收器
];

impl BatteryProtocol for McNewMouseProtocol {
    fn name(&self) -> &'static str {
        "K5"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        if info.vendor_id() != VID || !PIDS.contains(&info.product_id()) {
            return false;
        }

        // 只在厂商自定义集合上收发，避开鼠标/键盘标准集合
        let vendor_page = info.usage_page() >= 0xff00;
        if !vendor_page {
            return false;
        }

        let product = info.product_string().unwrap_or("").to_ascii_uppercase();
        product.contains("K5")
    }

    fn read_status(&self, device: &HidDevice) -> Result<DeviceStatus> {
        let (percent, charge_state) = read_battery(device)?;
        let dpi = read_dpi(device);

        Ok(DeviceStatus {
            battery: BatteryInfo {
                percent,
                charge_state,
            },
            dpi,
        })
    }
}

fn read_battery(device: &HidDevice) -> Result<(u8, ChargeState)> {
    for _ in 0..3 {
        let body = request(device, CMD_INFO, &[])?;
        if body.len() < 14 {
            bail!("K5 device info reply too short: {}", body.len());
        }

        let percent = body[12];
        if percent > 100 {
            bail!("invalid K5 battery percent: {percent}");
        }
        // HUB 在 batteryLevel=0 时最多重试 3 次（接收器刚唤醒时首包可能为 0）
        if percent > 0 {
            let charge_state = match body[11] {
                1 => ChargeState::Charging,
                2 if percent >= 95 => ChargeState::Full,
                _ if percent >= 100 => ChargeState::Full,
                _ => ChargeState::NotCharging,
            };
            return Ok((percent, charge_state));
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    bail!("K5 battery level stayed 0 after retries");
}

fn read_dpi(device: &HidDevice) -> Option<DpiInfo> {
    // 0x0002：body[0] 当前配置档
    let all_info = request(device, CMD_ALL_INFO, &[]).ok()?;
    let profile_index = *all_info.first()?;
    if profile_index > 2 {
        return None;
    }

    // 0x0003：data = [profileIndex, direction]；direction 0 = USB
    let body = request(device, CMD_DPI, &[profile_index, 0]).ok()?;
    if body.len() < 5 + 12 {
        return None;
    }

    let count = body[2];
    if count == 0 || count > 6 {
        return None;
    }

    let mut values = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let offset = 5 + i * 2;
        let value = u16::from_le_bytes([body[offset], body[offset + 1]]);
        if !(50..=50000).contains(&value) {
            return None;
        }
        values.push(value);
    }

    let index = (body[3] as usize).min(values.len() - 1);
    Some(DpiInfo {
        value: values[index],
        index: index as u8,
        count: values.len() as u8,
    })
}

fn request(device: &HidDevice, command: u16, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() > 55 {
        bail!("K5 command data too long: {}", data.len());
    }

    let mut report = vec![0u8; REPORT_LEN];
    report[0] = REPORT_ID;
    report[1] = 0x01; // version
    report[2] = 0x01; // flags（读命令固定 1，带 xor）
    report[3] = data.len() as u8;
    report[4] = (command & 0xff) as u8;
    report[5] = (command >> 8) as u8;
    report[6] = 0x00; // biz code
    report[7] = 0x00; // sequence
    report[8..8 + data.len()].copy_from_slice(data);
    report[8 + data.len()] = xor_sum(&report[2..8 + data.len()]);

    device.write(&report)?;

    for _ in 0..8 {
        let mut buf = [0u8; REPORT_LEN + 1];
        let len = device.read_timeout(&mut buf, 300)?;
        if len == 0 {
            continue;
        }
        if let Some(body) = parse_reply(&buf[..len], command) {
            return Ok(body.to_vec());
        }
    }

    bail!("no K5 reply for command {command:#06x}")
}

fn xor_sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, byte| acc ^ byte)
}

fn parse_reply<'a>(buf: &'a [u8], command: u16) -> Option<&'a [u8]> {
    // hidapi 对带编号的 report 会把 report id 放在首位
    let payload = if buf.first() == Some(&REPORT_ID) {
        &buf[1..]
    } else {
        buf
    };

    if payload.len() <= BODY_OFFSET {
        return None;
    }
    if payload[0] != 0x01 {
        return None;
    }
    // 命令号小端：cmdLo, cmdHi
    if payload[3] != (command & 0xff) as u8 || payload[4] != (command >> 8) as u8 {
        return None;
    }

    Some(&payload[BODY_OFFSET..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_reply(command: u16, body: &[u8]) -> Vec<u8> {
        // report id + payload(version..xor) + body
        let mut payload = vec![0u8; BODY_OFFSET + body.len() + 1];
        payload[0] = 0x01;
        payload[1] = 0x01;
        payload[2] = body.len() as u8;
        payload[3] = (command & 0xff) as u8;
        payload[4] = (command >> 8) as u8;
        payload[BODY_OFFSET..BODY_OFFSET + body.len()].copy_from_slice(body);

        let mut buf = vec![REPORT_ID];
        buf.extend_from_slice(&payload);
        buf
    }

    #[test]
    fn parses_k5_info_reply() {
        let mut body = [0u8; 14];
        body[11] = 0;
        body[12] = 79;
        let raw = build_reply(CMD_INFO, &body);
        let reply = parse_reply(&raw, CMD_INFO).expect("should parse");
        assert_eq!(reply[12], 79);
    }

    #[test]
    fn rejects_wrong_command() {
        let body = [0u8; 14];
        let wrong = build_reply(CMD_ALL_INFO, &body);
        assert!(parse_reply(&wrong, CMD_INFO).is_none());
    }

    #[test]
    fn dpi_reply_layout() {
        let mut body = vec![0u8; 5 + 12];
        body[0] = 0;
        body[2] = 6; // sum
        body[3] = 1; // 当前档
        body[4] = 0;
        for (i, value) in [400u16, 800, 1600, 3200, 6400, 26000].iter().enumerate() {
            body[5 + i * 2..7 + i * 2].copy_from_slice(&value.to_le_bytes());
        }
        let raw = build_reply(CMD_DPI, &body);
        let reply = parse_reply(&raw, CMD_DPI).expect("should parse");
        assert_eq!(reply[2], 6);
        assert_eq!(u16::from_le_bytes([reply[5], reply[6]]), 400);
        assert_eq!(u16::from_le_bytes([reply[15], reply[16]]), 26000);
    }
}
