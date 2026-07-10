use anyhow::{Context, Result};
use tray_icon::Icon;
use windows_registry::CURRENT_USER;

use crate::app_state::{AppearanceMode, BatteryInfo, ChargeState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconPalette {
    LightBackground,
    DarkBackground,
}

pub fn load_battery_icon(
    battery: Option<&BatteryInfo>,
    appearance: AppearanceMode,
) -> Result<Icon> {
    load_icon_from_bytes(battery_icon_bytes(battery, appearance))
}

pub fn load_menu_status_icon(battery: Option<&BatteryInfo>) -> Result<tray_icon::menu::Icon> {
    load_menu_icon_from_bytes(status_icon_bytes(battery))
}

pub fn load_menu_brand_icon(appearance: AppearanceMode) -> Result<tray_icon::menu::Icon> {
    load_menu_icon_from_bytes(brand_icon_bytes(appearance))
}

fn load_icon_from_bytes(bytes: &'static [u8]) -> Result<Icon> {
    let image = image::load_from_memory(bytes)
        .with_context(|| "failed to decode tray icon")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|err| anyhow::anyhow!("invalid tray icon: {err}"))
}

fn load_menu_icon_from_bytes(bytes: &'static [u8]) -> Result<tray_icon::menu::Icon> {
    let image = image::load_from_memory(bytes)
        .with_context(|| "failed to decode menu icon")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::menu::Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|err| anyhow::anyhow!("invalid menu icon: {err}"))
}

fn battery_icon_bytes(battery: Option<&BatteryInfo>, appearance: AppearanceMode) -> &'static [u8] {
    let palette = effective_palette(appearance);
    match battery {
        None => match palette {
            IconPalette::LightBackground => {
                include_bytes!("../assets/tray/png/battery-unknown-light.png")
            }
            IconPalette::DarkBackground => {
                include_bytes!("../assets/tray/png/battery-unknown-dark.png")
            }
        },
        Some(info) => {
            if matches!(info.charge_state, ChargeState::Charging) {
                match palette {
                    IconPalette::LightBackground => {
                        include_bytes!("../assets/tray/png/battery-charging-light.png")
                    }
                    IconPalette::DarkBackground => {
                        include_bytes!("../assets/tray/png/battery-charging-dark.png")
                    }
                }
            } else {
                match (battery_bucket(info.percent), palette) {
                    (0, IconPalette::LightBackground) => {
                        include_bytes!("../assets/tray/png/battery-0-light.png")
                    }
                    (0, IconPalette::DarkBackground) => {
                        include_bytes!("../assets/tray/png/battery-0-dark.png")
                    }
                    (1, IconPalette::LightBackground) => {
                        include_bytes!("../assets/tray/png/battery-1-light.png")
                    }
                    (1, IconPalette::DarkBackground) => {
                        include_bytes!("../assets/tray/png/battery-1-dark.png")
                    }
                    (2, IconPalette::LightBackground) => {
                        include_bytes!("../assets/tray/png/battery-2-light.png")
                    }
                    (2, IconPalette::DarkBackground) => {
                        include_bytes!("../assets/tray/png/battery-2-dark.png")
                    }
                    (_, IconPalette::LightBackground) => {
                        include_bytes!("../assets/tray/png/battery-3-light.png")
                    }
                    (_, IconPalette::DarkBackground) => {
                        include_bytes!("../assets/tray/png/battery-3-dark.png")
                    }
                }
            }
        }
    }
}

fn status_icon_bytes(battery: Option<&BatteryInfo>) -> &'static [u8] {
    match battery.map(|b| b.percent) {
        Some(0..=20) => include_bytes!("../assets/tray/png/battery-status-low.png"),
        Some(21..=50) => include_bytes!("../assets/tray/png/battery-status-medium.png"),
        Some(_) => include_bytes!("../assets/tray/png/battery-status-normal.png"),
        None => include_bytes!("../assets/tray/png/battery-status-unknown.png"),
    }
}

fn brand_icon_bytes(appearance: AppearanceMode) -> &'static [u8] {
    match effective_palette(appearance) {
        IconPalette::LightBackground => {
            include_bytes!("../assets/tray/png/mchose-logo-light-16.png")
        }
        IconPalette::DarkBackground => {
            include_bytes!("../assets/tray/png/mchose-logo-dark-16.png")
        }
    }
}

fn battery_bucket(percent: u8) -> u8 {
    match percent {
        0..=20 => 0,
        21..=50 => 1,
        51..=75 => 2,
        _ => 3,
    }
}

fn effective_palette(appearance: AppearanceMode) -> IconPalette {
    match appearance {
        AppearanceMode::Auto => {
            if system_uses_light_theme().unwrap_or(false) {
                IconPalette::LightBackground
            } else {
                IconPalette::DarkBackground
            }
        }
        AppearanceMode::Dark => IconPalette::DarkBackground,
        AppearanceMode::Light => IconPalette::LightBackground,
    }
}

fn system_uses_light_theme() -> Option<bool> {
    let key = CURRENT_USER
        .open("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .ok()?;
    let value: u32 = key.get_u32("SystemUsesLightTheme").ok()?;
    Some(value != 0)
}
