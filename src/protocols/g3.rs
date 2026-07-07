use anyhow::{bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState, DeviceStatus, DpiInfo};
use crate::protocols::BatteryProtocol;

pub struct G3Protocol;

const VID: u16 = 0xa8a5;
const PID: u16 = 0x2255;
const USAGE_PAGE: u16 = 0xff01;
const REPORT_LEN: usize = 65;

impl BatteryProtocol for G3Protocol {
    fn name(&self) -> &'static str {
        "G3"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        info.vendor_id() == VID && info.product_id() == PID && info.usage_page() == USAGE_PAGE
    }

    fn read_status(&self, device: &HidDevice) -> Result<DeviceStatus> {
        let (percent, charge_state) = read_battery_payload(device)?;
        let dpi = read_config_payload(device).map(|(value, index, count)| DpiInfo {
            value,
            index,
            count,
        });

        Ok(DeviceStatus {
            battery: BatteryInfo {
                percent,
                charge_state,
            },
            dpi,
        })
    }
}

fn read_battery_payload(device: &HidDevice) -> Result<(u8, ChargeState)> {
    let mut report = [0u8; REPORT_LEN];
    report[1] = 0x55;
    report[2] = 0x30;
    report[3] = 0xa5;
    report[4] = 0x0b;
    report[5] = 0x2e;
    report[6] = 0x01;
    report[7] = 0x01;
    report[8] = 0x01;

    device.write(&report)?;

    let response = read_response(device, 0xaa, 0x30)?;
    let offset = response_offset(&response, 0xaa, 0x30);

    let percent = response[offset + 8];
    if percent > 100 {
        bail!("invalid G3 battery percent: {percent}");
    }

    let charge_state = if response[offset + 9] == 0 {
        ChargeState::NotCharging
    } else {
        ChargeState::Charging
    };

    Ok((percent, charge_state))
}

fn read_config_payload(device: &HidDevice) -> Option<(u16, u8, u8)> {
    let mut report = [0u8; REPORT_LEN];
    report[1] = 0x55;
    report[2] = 0x0e;
    report[3] = 0xa5;
    report[4] = 0x0b;
    report[5] = 0x30;
    report[6] = 0x01;
    report[7] = 0x01;
    report[8] = 0x01;

    if device.write(&report).is_err() {
        return None;
    }

    let response = read_response(device, 0xaa, 0x0e).ok()?;
    let offset = response_offset(&response, 0xaa, 0x0e);

    if response.len() < offset + 25 {
        return None;
    }

    if response[offset + 13] == 0 && response[offset + 14] == 0 && response[offset + 15] == 0
        || response[offset + 13] == 0xff
            && response[offset + 14] == 0xff
            && response[offset + 15] == 0xff
    {
        return Some((2400, 2, 6));
    }

    let dpi_count = response[offset + 11];
    if dpi_count == 0 || dpi_count > 6 {
        return None;
    }

    let mut raw_index = response[offset + 12].saturating_sub(1);
    if raw_index >= dpi_count {
        raw_index = 0;
    }

    let value_offset = offset + 13 + (raw_index as usize) * 2;
    if value_offset + 1 >= response.len() {
        return None;
    }

    let mut dpi_value = response[value_offset] as u16 | ((response[value_offset + 1] as u16) << 8);
    if dpi_value < 50 {
        dpi_value = 50;
    }
    if dpi_value > 50000 {
        dpi_value = 50000;
    }

    Some((dpi_value, raw_index, dpi_count))
}

fn read_response(device: &HidDevice, prefix0: u8, prefix1: u8) -> Result<Vec<u8>> {
    for _ in 0..8 {
        let mut buf = [0u8; REPORT_LEN];
        let len = device.read_timeout(&mut buf, 1_000)?;
        if len == 0 {
            continue;
        }

        let slice = &buf[..len];
        if matches_prefix(slice, prefix0, prefix1) {
            return Ok(slice.to_vec());
        }
    }

    bail!("no matching response for prefix {prefix0:#04x} {prefix1:#04x}");
}

fn matches_prefix(response: &[u8], prefix0: u8, prefix1: u8) -> bool {
    response.len() >= 3 && response[0] == prefix0 && response[1] == prefix1
        || response.len() >= 4 && response[1] == prefix0 && response[2] == prefix1
}

fn response_offset(response: &[u8], prefix0: u8, prefix1: u8) -> usize {
    if response[0] == prefix0 && response[1] == prefix1 {
        0
    } else {
        1
    }
}
