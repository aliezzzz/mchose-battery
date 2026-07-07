use anyhow::{bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState};
use crate::protocols::BatteryProtocol;

pub struct G3Protocol;

const VID: u16 = 0xa8a5;
const PID: u16 = 0x2255;
const USAGE_PAGE: u16 = 0xff01;
const USAGE: u16 = 0x0010;
const REPORT_LEN: usize = 65;

impl BatteryProtocol for G3Protocol {
    fn name(&self) -> &'static str {
        "G3"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        info.vendor_id() == VID
            && info.product_id() == PID
            && info.usage_page() == USAGE_PAGE
            && info.usage() == USAGE
    }

    fn read_battery(&self, device: &HidDevice) -> Result<BatteryInfo> {
        read_g3_battery(device)
    }
}

fn read_g3_battery(device: &HidDevice) -> Result<BatteryInfo> {
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

    let mut buf = [0u8; REPORT_LEN];
    let len = device.read_timeout(&mut buf, 1_000)?;
    parse_g3_response(&buf[..len])
}

fn parse_g3_response(response: &[u8]) -> Result<BatteryInfo> {
    let offset = if response.len() >= 10 && response[0] == 0xaa && response[1] == 0x30 {
        0
    } else if response.len() >= 11 && response[1] == 0xaa && response[2] == 0x30 {
        1
    } else {
        bail!("unexpected G3 response: {:02x?}", response);
    };

    let percent = response[offset + 8];
    if percent > 100 {
        bail!("invalid G3 battery percent: {percent}");
    }

    let charge_state = if response[offset + 9] == 0 {
        ChargeState::NotCharging
    } else {
        ChargeState::Charging
    };

    Ok(BatteryInfo {
        percent,
        charge_state,
    })
}
