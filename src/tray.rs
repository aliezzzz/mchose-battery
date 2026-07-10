use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use tray_icon::{
    menu::{CheckMenuItem, IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use winit::event_loop::EventLoopProxy;

use crate::app_state::{AppState, AppearanceMode};
use crate::icons::{load_battery_icon, load_menu_brand_icon, load_menu_status_icon};
use crate::UserEvent;

pub const MENU_REFRESH: &str = "refresh";
pub const MENU_OPEN_WEBSITE: &str = "open_website";
pub const MENU_RESCAN: &str = "rescan";
pub const MENU_APPEARANCE_AUTO: &str = "appearance_auto";
pub const MENU_APPEARANCE_DARK: &str = "appearance_dark";
pub const MENU_APPEARANCE_LIGHT: &str = "appearance_light";
pub const MENU_STARTUP: &str = "startup";
pub const MENU_QUIT: &str = "quit";

pub struct TrayController {
    _menu: Menu,
    tray_icon: TrayIcon,
    title_item: IconMenuItem,
    current_item: MenuItem,
    battery_item: IconMenuItem,
    status_item: MenuItem,
    dpi_item: MenuItem,
    appearance_auto_item: CheckMenuItem,
    appearance_dark_item: CheckMenuItem,
    appearance_light_item: CheckMenuItem,
    startup_item: CheckMenuItem,
}

impl TrayController {
    pub fn new(state: Arc<Mutex<AppState>>, proxy: EventLoopProxy<UserEvent>) -> Result<Self> {
        let title_item = {
            let state = state.lock();
            IconMenuItem::with_id(
                MENU_OPEN_WEBSITE,
                "MCHOSE Battery",
                true,
                Some(load_menu_brand_icon(state.appearance_mode)?),
                None,
            )
        };
        let current_item = MenuItem::with_id("current", "● 未发现设备", true, None);
        let battery_item = IconMenuItem::with_id("battery", "电量：--", true, None, None);
        let status_item = MenuItem::with_id("status", "状态：--", true, None);
        let dpi_item = MenuItem::with_id("dpi", "DPI：--", true, None);
        let refresh_item = MenuItem::with_id(MENU_REFRESH, "刷新", true, None);
        let rescan_item = MenuItem::with_id(MENU_RESCAN, "重新扫描设备", true, None);
        let appearance_auto_item =
            CheckMenuItem::with_id(MENU_APPEARANCE_AUTO, "自动", true, true, None);
        let appearance_dark_item =
            CheckMenuItem::with_id(MENU_APPEARANCE_DARK, "深色", true, false, None);
        let appearance_light_item =
            CheckMenuItem::with_id(MENU_APPEARANCE_LIGHT, "浅色", true, false, None);
        let appearance_menu = Submenu::with_items(
            "外观",
            true,
            &[
                &appearance_auto_item,
                &appearance_dark_item,
                &appearance_light_item,
            ],
        )?;
        let startup_item = {
            let state = state.lock();
            CheckMenuItem::with_id(MENU_STARTUP, "开机启动", true, state.startup_enabled, None)
        };
        let quit_item = MenuItem::with_id(MENU_QUIT, "退出", true, None);

        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();

        let menu = Menu::new();
        menu.append_items(&[
            &title_item,
            &sep1,
            &current_item,
            &battery_item,
            &status_item,
            &dpi_item,
            &sep2,
            &refresh_item,
            &rescan_item,
            &appearance_menu,
            &startup_item,
            &quit_item,
        ])?;

        let icon = {
            let state = state.lock();
            load_battery_icon(
                state
                    .current_device()
                    .and_then(|device| device.status.as_ref())
                    .map(|status| &status.battery),
                state.appearance_mode,
            )?
        };

        let tooltip = state.lock().current_tooltip();

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .build()?;

        setup_event_forwarding(proxy);

        Ok(Self {
            _menu: menu,
            tray_icon,
            title_item,
            current_item,
            battery_item,
            status_item,
            dpi_item,
            appearance_auto_item,
            appearance_dark_item,
            appearance_light_item,
            startup_item,
        })
    }

    pub fn refresh_all(&self, state: &AppState) -> Result<()> {
        self.title_item
            .set_icon(Some(load_menu_brand_icon(state.appearance_mode)?));

        match state.current_device() {
            Some(device) => {
                self.current_item.set_text(format!("● {}", device.name));
                let battery = device.status.as_ref().map(|status| &status.battery);
                self.battery_item
                    .set_text(format_battery_menu_text(battery));
                self.battery_item
                    .set_icon(Some(load_menu_status_icon(battery)?));
                self.status_item.set_text(format!(
                    "状态：{}",
                    format_charge_label(device.charge_label())
                ));
                self.dpi_item.set_text(device.dpi_label());

                self.tray_icon
                    .set_icon(Some(load_battery_icon(battery, state.appearance_mode)?))?;
            }
            None => {
                self.current_item.set_text("● 未发现设备");
                self.battery_item.set_text("电量：--");
                self.battery_item
                    .set_icon(Some(load_menu_status_icon(None)?));
                self.status_item.set_text("状态：--");
                self.dpi_item.set_text("DPI：未知");

                self.tray_icon
                    .set_icon(Some(load_battery_icon(None, state.appearance_mode)?))?;
            }
        }

        self.refresh_appearance_menu(state.appearance_mode);
        self.startup_item.set_checked(state.startup_enabled);
        self.tray_icon.set_tooltip(Some(&state.current_tooltip()))?;
        Ok(())
    }

    fn refresh_appearance_menu(&self, appearance: AppearanceMode) {
        self.appearance_auto_item
            .set_checked(appearance == AppearanceMode::Auto);
        self.appearance_dark_item
            .set_checked(appearance == AppearanceMode::Dark);
        self.appearance_light_item
            .set_checked(appearance == AppearanceMode::Light);
    }
}

fn format_battery_menu_text(battery: Option<&crate::app_state::BatteryInfo>) -> String {
    match battery {
        Some(battery) => {
            let warning = if battery.percent <= 10 {
                "  低电量"
            } else {
                ""
            };
            format!(
                "电量：{}%  {}{}",
                battery.percent,
                battery_bar(battery.percent),
                warning
            )
        }
        None => "电量：未知  ▱▱▱▱▱▱▱▱▱▱".to_string(),
    }
}

fn battery_bar(percent: u8) -> String {
    let filled = ((percent.min(100) as usize) + 9) / 10;
    let empty = 10 - filled;
    format!("{}{}", "▰".repeat(filled), "▱".repeat(empty))
}

fn format_charge_label(label: &str) -> String {
    match label {
        "充电中" => "⚡ 充电中".to_string(),
        "已充满" => "✓ 已充满".to_string(),
        "未知" => "? 未知".to_string(),
        _ => label.to_string(),
    }
}

fn setup_event_forwarding(proxy: EventLoopProxy<UserEvent>) {
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button,
            button_state,
            ..
        } = event
        {
            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                let _ = tray_proxy.send_event(UserEvent::TrayLeftClick);
            }
        }
    }));

    let menu_proxy = proxy;
    tray_icon::menu::MenuEvent::set_event_handler(Some(
        move |event: tray_icon::menu::MenuEvent| {
            let id = event.id.as_ref();
            match id {
                MENU_OPEN_WEBSITE => {
                    let _ = menu_proxy.send_event(UserEvent::OpenWebsite);
                }
                MENU_REFRESH => {
                    let _ = menu_proxy.send_event(UserEvent::Refresh);
                }
                MENU_RESCAN => {
                    let _ = menu_proxy.send_event(UserEvent::Rescan);
                }
                MENU_APPEARANCE_AUTO => {
                    let _ = menu_proxy.send_event(UserEvent::SetAppearance(AppearanceMode::Auto));
                }
                MENU_APPEARANCE_DARK => {
                    let _ = menu_proxy.send_event(UserEvent::SetAppearance(AppearanceMode::Dark));
                }
                MENU_APPEARANCE_LIGHT => {
                    let _ = menu_proxy.send_event(UserEvent::SetAppearance(AppearanceMode::Light));
                }
                MENU_STARTUP => {
                    let _ = menu_proxy.send_event(UserEvent::ToggleStartup);
                }
                MENU_QUIT => {
                    let _ = menu_proxy.send_event(UserEvent::Exit);
                }
                _ => {}
            }
        },
    ));
}
