#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod device_listener;
mod hid_scan;
mod icons;
mod protocols;
mod tray;
mod windows_theme;

use std::sync::Arc;
use std::time::Duration;

use app_state::{AppState, AppearanceMode};
use parking_lot::Mutex;
use tray::TrayController;
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};

#[derive(Debug)]
pub enum UserEvent {
    TrayLeftClick,
    Refresh,
    Rescan,
    SetAppearance(AppearanceMode),
    Exit,
    Tick,
}

fn main() -> anyhow::Result<()> {
    windows_theme::apply_menu_appearance(AppearanceMode::Auto);

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--scan") {
        // Debug 模式保留控制台输出。release 下如果 windows_subsystem = windows，
        // 建议单独加一个 console feature 或临时注释掉上面的 cfg_attr。
        hid_scan::debug_scan()?;
        return Ok(());
    }

    run_tray()
}

fn run_tray() -> anyhow::Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let state = Arc::new(Mutex::new(AppState::new()));

    {
        let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
        state.lock().set_devices(devices);
    }

    let tray = TrayController::new(state.clone(), proxy.clone())?;
    tray.refresh_all(&state.lock())?;

    let tick_proxy = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30 * 60));
        let _ = tick_proxy.send_event(UserEvent::Tick);
    });

    device_listener::spawn_device_listener(proxy.clone());

    event_loop.run(move |event, event_loop| {
        event_loop.set_control_flow(ControlFlow::Wait);

        match event {
            Event::UserEvent(UserEvent::TrayLeftClick) => {
                {
                    let mut state = state.lock();
                    if state.devices.len() <= 1 {
                        let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
                        state.set_devices(devices);
                    } else {
                        state.switch_next();
                    }
                }
                let _ = tray.refresh_all(&state.lock());
            }
            Event::UserEvent(UserEvent::Refresh) | Event::UserEvent(UserEvent::Tick) => {
                {
                    let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
                    state.lock().set_devices(devices);
                }
                let _ = tray.refresh_all(&state.lock());
            }
            Event::UserEvent(UserEvent::Rescan) => {
                {
                    let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
                    let mut state = state.lock();
                    state.set_devices(devices);
                    state.current_index = 0;
                }
                let _ = tray.refresh_all(&state.lock());
            }
            Event::UserEvent(UserEvent::SetAppearance(appearance)) => {
                {
                    let mut state = state.lock();
                    state.appearance_mode = appearance;
                }
                windows_theme::apply_menu_appearance(appearance);
                let _ = tray.refresh_all(&state.lock());
            }
            Event::UserEvent(UserEvent::Exit) => {
                event_loop.exit();
            }
            _ => {}
        }
    })?;

    Ok(())
}
