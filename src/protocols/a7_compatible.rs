use anyhow::{anyhow, bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState};
use crate::protocols::BatteryProtocol;

pub struct A7CompatibleProtocol;

const VID: u16 = 0x5253;
const PID: u16 = 0x1021;
const REPORT_ID: u8 = 0x11;
const REPORT_LEN: usize = 64;

impl BatteryProtocol for A7CompatibleProtocol {
    fn name(&self) -> &'static str {
        "A7-compatible"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        // 已知 A7 设备来自公开项目：VID 0x5253, PID 0x1021。
        info.vendor_id() == VID && info.product_id() == PID
    }

    fn read_battery(&self, device: &HidDevice) -> Result<BatteryInfo> {
        read_a7_compatible_battery(device)
    }
}

pub fn read_a7_compatible_battery(device: &HidDevice) -> Result<BatteryInfo> {
    let report = build_query_report();
    device.send_feature_report(&report)?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut buf = [0u8; REPORT_LEN];
    buf[0] = REPORT_ID;

    let len = device.get_feature_report(&mut buf)?;
    parse_response(&buf[..len])
}

fn build_query_report() -> Vec<u8> {
    // JavaScript 原逻辑：
    // dataPart = [0x06, ...Array(19).fill(0x00)]
    // sendData = dataPart.map(b => b ^ 0xFF)
    // sendBuffer = [reportId, ...sendData]
    let mut data_part = vec![0x06];
    data_part.extend(std::iter::repeat(0x00).take(19));

    let mut report = vec![REPORT_ID];
    for b in data_part {
        report.push(b ^ 0xff);
    }
    report
}

fn parse_response(response: &[u8]) -> Result<BatteryInfo> {
    if response.len() < 13 {
        bail!("response too short: {}", response.len());
    }

    let raw: Vec<u8> = response.iter().map(|b| b ^ 0xff).collect();
    let payload = raw.get(2..).ok_or_else(|| anyhow!("invalid payload"))?;

    if payload.len() < 11 {
        bail!("payload too short: {}", payload.len());
    }

    let percent = payload[9];
    let charge_status = payload[10];

    if percent > 100 {
        bail!("invalid battery percent: {}", percent);
    }

    let charge_state = match charge_status {
        0 => ChargeState::NotCharging,
        1 => ChargeState::Charging,
        2 => ChargeState::Full,
        _ => ChargeState::Unknown,
    };

    Ok(BatteryInfo {
        percent,
        charge_state,
    })
}
