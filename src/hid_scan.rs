use std::time::Instant;

use anyhow::Result;
use hidapi::{DeviceInfo, HidApi};

use crate::app_state::MchoseDevice;
use crate::protocols::{all_protocols, try_read_with_any_protocol};

pub fn scan_mchose_devices() -> Result<Vec<MchoseDevice>> {
    let api = HidApi::new()?;
    let mut result = Vec::new();

    for info in api.device_list() {
        if !looks_like_mchose(info) {
            continue;
        }

        if let Some((protocol, battery)) = try_read_with_any_protocol(&api, info) {
            let path = info.path().to_string_lossy().to_string();
            let product = info.product_string().unwrap_or("").trim().to_string();

            let name = if product.is_empty() {
                format!("MCHOSE {:04x}:{:04x}", info.vendor_id(), info.product_id())
            } else {
                product
            };

            result.push(MchoseDevice {
                id: stable_id_from_path(&path),
                name,
                vid: info.vendor_id(),
                pid: info.product_id(),
                path,
                protocol: protocol.to_string(),
                battery: Some(battery),
                last_seen: Instant::now(),
            });
        }
    }

    Ok(result)
}

pub fn debug_scan() -> Result<()> {
    let api = HidApi::new()?;

    println!("=== HID devices that look like MCHOSE / MaiCong ===");

    for info in api.device_list() {
        if !looks_like_mchose(info) {
            continue;
        }

        println!("\n[DEVICE]");
        println!("manufacturer = {:?}", info.manufacturer_string());
        println!("product      = {:?}", info.product_string());
        println!(
            "vid:pid      = {:04x}:{:04x}",
            info.vendor_id(),
            info.product_id()
        );
        println!("usage_page   = {:?}", info.usage_page());
        println!("usage        = {:?}", info.usage());
        println!("interface    = {:?}", info.interface_number());
        println!("path         = {}", info.path().to_string_lossy());

        match try_read_with_any_protocol(&api, info) {
            Some((protocol, battery)) => {
                println!("[OK] protocol = {}", protocol);
                println!("[OK] battery  = {}%", battery.percent);
                println!("[OK] charge   = {}", battery.charge_state.label());
            }
            None => {
                println!("[FAIL] no known protocol returned valid battery data");
                for protocol in all_protocols() {
                    if !protocol.matches(info) {
                        continue;
                    }

                    match info.open_device(&api) {
                        Ok(device) => match protocol.read_battery(&device) {
                            Ok(_) => {}
                            Err(err) => println!("[DEBUG] {} error: {err}", protocol.name()),
                        },
                        Err(err) => println!("[DEBUG] {} open error: {err}", protocol.name()),
                    }
                }
            }
        }
    }

    Ok(())
}

fn looks_like_mchose(info: &DeviceInfo) -> bool {
    let manufacturer = info
        .manufacturer_string()
        .unwrap_or("")
        .to_ascii_lowercase();
    let product = info.product_string().unwrap_or("").to_ascii_lowercase();
    let text = format!(
        "{} {} {:04x}:{:04x}",
        manufacturer,
        product,
        info.vendor_id(),
        info.product_id()
    );

    text.contains("mchose")
        || text.contains("maicong")
        || text.contains("迈从")
        || info.vendor_id() == 0x5253
}

fn stable_id_from_path(path: &str) -> String {
    // 简单 FNV-1a，避免菜单 item id 太长。
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in path.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("dev_{hash:016x}")
}
