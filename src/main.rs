#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod device_listener;
mod hid_scan;
mod icons;
mod protocols;
mod scan_worker;
mod startup;
mod tray;
mod windows_theme;

use std::sync::Arc;
use std::time::Duration;

use app_state::{AppState, AppearanceMode, MchoseDevice};
use parking_lot::Mutex;
use scan_worker::ScanWorker;
use tray::TrayController;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

/// 结束本线程上的 TrackPopupMenu（须在菜单所在线程调用）。
fn end_popup_menu() {
    unsafe {
        extern "system" {
            fn EndMenu() -> i32;
        }
        let _ = EndMenu();
    }
}

#[derive(Debug)]
pub enum UserEvent {
    TrayLeftClick,
    TrayRightClick,
    OpenWebsite,
    Refresh,
    Rescan,
    SetAppearance(AppearanceMode),
    ToggleStartup,
    Exit,
    Tick,
    /// USB/HID 热插拔：取消模态菜单并安排扫描
    DeviceHotplug,
    DevicesUpdated(Vec<MchoseDevice>),
}

struct TrayApp {
    state: Arc<Mutex<AppState>>,
    tray: TrayController,
    scan_worker: ScanWorker,
    menu_open: bool,
    /// 菜单打开期间有状态变化，关闭后需 refresh
    ui_dirty: bool,
    /// 菜单打开期间推迟扫描
    pending_scan: bool,
}

impl TrayApp {
    fn request_scan(&mut self) {
        if self.menu_open {
            self.pending_scan = true;
        } else {
            self.scan_worker.request_scan();
        }
    }

    fn refresh_ui(&mut self) {
        let _ = self.tray.refresh_all(&self.state.lock());
        self.ui_dirty = false;
    }

    /// 菜单关闭后：补 UI + 补扫描
    fn after_menu_closed(&mut self) {
        if self.ui_dirty {
            self.refresh_ui();
        }
        if self.pending_scan {
            self.pending_scan = false;
            self.scan_worker.request_scan();
        }
    }
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(
        &mut self,
        _: &ActiveEventLoop,
        _: winit::window::WindowId,
        _: winit::event::WindowEvent,
    ) {
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::TrayLeftClick => {
                let need_scan = {
                    let mut state = self.state.lock();
                    if state.devices.len() <= 1 {
                        true
                    } else {
                        state.switch_next();
                        false
                    }
                };
                if need_scan {
                    self.request_scan();
                } else {
                    self.refresh_ui();
                }
            }
            UserEvent::TrayRightClick => {
                self.refresh_ui();
                self.menu_open = true;
                self.tray.show_menu(); // TrackPopupMenu 阻塞；热插拔会 EndMenu 打断
                self.menu_open = false;
                self.after_menu_closed();
            }
            UserEvent::OpenWebsite => {
                if let Err(err) = open_website() {
                    eprintln!("failed to open MCHOSE website: {err}");
                }
            }
            UserEvent::Refresh | UserEvent::Tick => self.request_scan(),
            UserEvent::DeviceHotplug => {
                if self.menu_open {
                    end_popup_menu();
                    self.pending_scan = true;
                    self.ui_dirty = true;
                } else {
                    self.scan_worker.request_scan();
                }
            }
            UserEvent::Rescan => {
                self.state.lock().current_index = 0;
                self.request_scan();
            }
            UserEvent::DevicesUpdated(devices) => {
                self.state.lock().set_devices(devices);
                if self.menu_open {
                    self.ui_dirty = true;
                } else {
                    self.refresh_ui();
                }
            }
            UserEvent::SetAppearance(appearance) => {
                self.state.lock().appearance_mode = appearance;
                windows_theme::apply_menu_appearance(appearance);
                self.refresh_ui();
            }
            UserEvent::ToggleStartup => {
                let enabled = !self.state.lock().startup_enabled;
                match startup::set_enabled(enabled) {
                    Ok(()) => self.state.lock().startup_enabled = enabled,
                    Err(err) => eprintln!("failed to update startup setting: {err}"),
                }
                self.refresh_ui();
            }
            UserEvent::Exit => event_loop.exit(),
        }
    }
}

fn open_website() -> anyhow::Result<()> {
    let url: Vec<u16> = "https://www.mchose.com.cn\0".encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };

    if (result as isize) <= 32 {
        anyhow::bail!("ShellExecuteW failed with code {}", result as isize);
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    windows_theme::apply_menu_appearance(AppearanceMode::Auto);

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--scan") {
        hid_scan::debug_scan()?;
        return Ok(());
    }

    run_tray()
}

fn run_tray() -> anyhow::Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let state = Arc::new(Mutex::new(AppState::new()));
    state.lock().startup_enabled = startup::is_enabled().unwrap_or_else(|err| {
        eprintln!("failed to read startup setting: {err}");
        false
    });

    {
        let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
        state.lock().set_devices(devices);
    }

    let tray = TrayController::new(state.clone(), proxy.clone())?;
    tray.refresh_all(&state.lock())?;

    let scan_worker = ScanWorker::spawn(proxy.clone());

    let tick_proxy = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30 * 60));
        let _ = tick_proxy.send_event(UserEvent::Tick);
    });

    device_listener::spawn_device_listener(proxy);

    let mut app = TrayApp {
        state,
        tray,
        scan_worker,
        menu_open: false,
        ui_dirty: false,
        pending_scan: false,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
