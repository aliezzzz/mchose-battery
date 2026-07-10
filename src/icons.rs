use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tray_icon::Icon;
use windows_registry::CURRENT_USER;

use crate::app_state::{AppearanceMode, BatteryInfo, ChargeState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconPalette {
    LightBackground,
    DarkBackground,
}

pub fn icon_path_for_battery(battery: Option<&BatteryInfo>, appearance: AppearanceMode) -> PathBuf {
    let palette = effective_palette(appearance);
    let suffix = match palette {
        IconPalette::LightBackground => "light",
        IconPalette::DarkBackground => "dark",
    };

    let name = match battery {
        None => format!("battery-unknown-{suffix}.png"),
        Some(info) => {
            if matches!(info.charge_state, ChargeState::Charging) {
                format!("battery-charging-{suffix}.png")
            } else {
                let bucket = battery_bucket(info.percent);
                format!("battery-{bucket}-{suffix}.png")
            }
        }
    };

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("tray")
        .join("png")
        .join(name)
}

pub fn load_icon(path: impl AsRef<Path>) -> Result<Icon> {
    let path = path.as_ref();
    let image = image::open(path)
        .with_context(|| format!("failed to load tray icon: {}", path.display()))?
        .into_rgba8();

    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|err| anyhow::anyhow!("invalid tray icon {}: {err}", path.display()))
}

pub fn load_battery_icon(
    battery: Option<&BatteryInfo>,
    appearance: AppearanceMode,
) -> Result<Icon> {
    let path = icon_path_for_battery(battery, appearance);
    load_icon(path)
}

pub fn load_menu_status_icon(battery: Option<&BatteryInfo>) -> Result<tray_icon::menu::Icon> {
    let path = status_icon_path(battery);
    load_menu_icon_from_path(path)
}

pub fn load_menu_brand_icon(appearance: AppearanceMode) -> Result<tray_icon::menu::Icon> {
    let suffix = match effective_palette(appearance) {
        IconPalette::LightBackground => "light",
        IconPalette::DarkBackground => "dark",
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("tray")
        .join("png")
        .join(format!("mchose-logo-{suffix}-16.png"));
    load_menu_icon_from_path(path)
}

fn load_menu_icon_from_path(path: PathBuf) -> Result<tray_icon::menu::Icon> {
    let image = image::open(&path)
        .with_context(|| format!("failed to load menu icon: {}", path.display()))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::menu::Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|err| anyhow::anyhow!("invalid menu icon {}: {err}", path.display()))
}

fn status_icon_path(battery: Option<&BatteryInfo>) -> PathBuf {
    let name = match battery.map(|battery| battery.percent) {
        Some(0..=20) => "battery-status-low.png",
        Some(21..=50) => "battery-status-medium.png",
        Some(_) => "battery-status-normal.png",
        None => "battery-status-unknown.png",
    };

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("tray")
        .join("png")
        .join(name)
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

pub fn system_uses_light_theme() -> Option<bool> {
    let key = CURRENT_USER
        .open("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .ok()?;
    let value: u32 = key.get_u32("SystemUsesLightTheme").ok()?;
    Some(value != 0)
}
