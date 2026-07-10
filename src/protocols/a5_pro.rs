use anyhow::{bail, Result};
use hidapi::{DeviceInfo, HidDevice};

use crate::app_state::{BatteryInfo, ChargeState, DeviceStatus, DpiInfo};
use crate::protocols::BatteryProtocol;

pub struct A5ProProtocol;

const VID: u16 = 0x2023;
const REPORT_LEN: usize = 65;
const CMD_BATTERY: u8 = 0x83;
const CMD_DPI_STAGES: u8 = 0x81;
const CMD_ACTIVE_DPI: u8 = 0x82;
const DEFAULT_PROFILE: u8 = 1;
const MAX_DPI_STAGES: u8 = 6;

/// A5 / AX5 系列 PID（来自旧驱动 Config/xvi_models.xlsx）
const PIDS: &[u16] = &[
    0xf012, // A5 Pro wireless bootloader
    0xf013, // A5 Pro / Pro Max wireless dongle
    0xf015, // A5 Pro golden dongle
    0xf016, // A5 Pro wired bootloader
    0xf017, // A5 Pro wired
    0xf018, // A5 Pro Max wired bootloader
    0xf019, // A5 Pro Max wired
    0xf021, // AX5 Pro Max wireless bootloader
    0xf022, // AX5 Pro Max wireless
    0xf023, // AX5 Pro wireless bootloader / AX5 Pro Max wired bootloader
    0xf024, // AX5 Pro wireless / AX5 Pro Max wired
    0xf025, // AX5 Pro wired bootloader
    0xf026, // AX5 Pro wired
];

impl BatteryProtocol for A5ProProtocol {
    fn name(&self) -> &'static str {
        "A5/AX5"
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        // 旧驱动 xvi_models 指定厂商接口 MI02；feature report 长度 65。
        info.vendor_id() == VID
            && PIDS.contains(&info.product_id())
            && info.interface_number() == 2
    }

    fn read_status(&self, device: &HidDevice) -> Result<DeviceStatus> {
        read_a5_status(device)
    }
}

pub fn read_a5_status(device: &HidDevice) -> Result<DeviceStatus> {
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

fn read_battery(device: &HidDevice) -> Result<(u8, ChargeState)> {
    let response = feature_xfer(device, |report| {
        report[4] = 0x02;
        report[6] = CMD_BATTERY;
    })?;

    // UI 用 GetBat(ref charge, ref percent)：response[7]=充电, response[8]=电量
    let charge_raw = response[7];
    let percent = response[8];
    if percent > 100 {
        bail!("invalid A5 battery percent: {percent}");
    }

    let charge_state = match charge_raw {
        0 => ChargeState::NotCharging,
        1 => ChargeState::Charging,
        2 => ChargeState::Full,
        _ => ChargeState::Unknown,
    };

    Ok((percent, charge_state))
}

fn read_dpi(device: &HidDevice) -> Option<DpiInfo> {
    // GetDPIStageInfo 第一步：cmd 0x81 取档位表
    let stages = feature_xfer(device, |report| {
        report[4] = 0x0a;
        report[5] = 0x01;
        report[6] = CMD_DPI_STAGES;
        report[7] = DEFAULT_PROFILE;
        report[8] = MAX_DPI_STAGES;
    })
    .ok()?;

    let count = stages[8];
    if count == 0 || count > MAX_DPI_STAGES {
        return None;
    }

    let mut values = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let hi = stages[9 + i * 4] as u16;
        let lo = stages[10 + i * 4] as u16;
        let value = (hi << 8) | lo;
        if value == 0 {
            continue;
        }
        values.push(value);
    }

    if values.is_empty() {
        return None;
    }

    // 第二步：cmd 0x82 取当前档
    let active = feature_xfer(device, |report| {
        report[4] = 0x02;
        report[5] = 0x01;
        report[6] = CMD_ACTIVE_DPI;
        report[7] = DEFAULT_PROFILE;
    })
    .ok()?;

    let raw_index = active[8];
    // 驱动返回多为 1-based 档位；兼容 0-based
    let index = if raw_index == 0 {
        0
    } else if (raw_index as usize) <= values.len() {
        raw_index.saturating_sub(1)
    } else {
        0
    };

    let index = (index as usize).min(values.len() - 1) as u8;
    let value = values[index as usize];

    Some(DpiInfo {
        value,
        index,
        count: values.len() as u8,
    })
}

/// DriverCore feature 往返：SetFeature → 等 50ms → GetFeature。
/// 收发时 [3]=2（驱动 VVqCXhEjOX / sxkC74VqY8 原逻辑）。
fn feature_xfer(device: &HidDevice, fill: impl FnOnce(&mut [u8])) -> Result<Vec<u8>> {
    let mut report = vec![0u8; REPORT_LEN];
    fill(&mut report);
    report[3] = 0x02;

    device.send_feature_report(&report)?;
    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut response = vec![0u8; REPORT_LEN];
    response[3] = 0x02;
    let len = device.get_feature_report(&mut response)?;
    if len < 9 {
        bail!("A5 feature response too short: {len}");
    }

    Ok(response)
}
