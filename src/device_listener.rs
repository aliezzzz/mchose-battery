use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

const WM_DEVICECHANGE: u32 = 0x0219;
const DBT_DEVICEARRIVAL: u32 = 0x8000;
const DBT_DEVICEREMOVECOMPLETE: u32 = 0x8004;
const DBT_DEVTYP_DEVICEINTERFACE: u32 = 0x00000005;

#[repr(C)]
struct DevBroadcastDeviceInterface {
    dbcc_size: u32,
    dbcc_devicetype: u32,
    dbcc_reserved: u32,
    dbcc_classguid: [u8; 16],
    dbcc_name: [u16; 1],
}

extern "system" {
    fn CreateWindowExW(
        dw_ex_style: u32,
        lp_class_name: *const u16,
        lp_window_name: *const u16,
        dw_style: u32,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: isize,
        h_menu: isize,
        h_instance: isize,
        lp_param: *const std::ffi::c_void,
    ) -> isize;

    fn DefWindowProcW(h_wnd: isize, msg: u32, w_param: usize, l_param: isize) -> isize;

    fn RegisterClassW(lp_wnd_class: *const WndClassW) -> u16;

    fn RegisterDeviceNotificationW(
        h_recipient: isize,
        notification_filter: *const std::ffi::c_void,
        flags: u32,
    ) -> isize;

    fn UnregisterDeviceNotification(handle: isize) -> i32;

    fn DestroyWindow(h_wnd: isize) -> i32;

    fn GetMessageW(
        lp_msg: *mut Msg,
        h_wnd: isize,
        w_msg_filter_min: u32,
        w_msg_filter_max: u32,
    ) -> i32;

    fn TranslateMessage(lp_msg: *const Msg) -> i32;

    fn DispatchMessageW(lp_msg: *const Msg) -> isize;

    fn GetModuleHandleW(lp_module_name: *const u16) -> isize;

    fn FindWindowW(lp_class_name: *const u16, lp_window_name: *const u16) -> isize;

    fn PostMessageW(h_wnd: isize, msg: u32, w_param: usize, l_param: isize) -> i32;

    fn EnumWindows(
        lp_enum_func: unsafe extern "system" fn(isize, isize) -> i32,
        l_param: isize,
    ) -> i32;

    fn GetClassNameW(h_wnd: isize, lp_class_name: *mut u16, n_max_count: i32) -> i32;
}

const WM_CANCELMODE: u32 = 0x001F;

/// 跨线程取消 tray-icon 的 TrackPopupMenu（窗口类名 tray_icon_app）。
unsafe extern "system" fn enum_cancel_tray_menu(h_wnd: isize, _l_param: isize) -> i32 {
    let mut class = [0u16; 64];
    let len = GetClassNameW(h_wnd, class.as_mut_ptr(), class.len() as i32);
    if len > 0 {
        let name = String::from_utf16_lossy(&class[..len as usize]);
        if name == "tray_icon_app" {
            let _ = PostMessageW(h_wnd, WM_CANCELMODE, 0, 0);
        }
    }
    1 // continue
}

fn cancel_tray_popup_menus() {
    unsafe {
        let class: Vec<u16> = "tray_icon_app\0".encode_utf16().collect();
        let hwnd = FindWindowW(class.as_ptr(), std::ptr::null());
        if hwnd != 0 {
            let _ = PostMessageW(hwnd, WM_CANCELMODE, 0, 0);
        }
        let _ = EnumWindows(enum_cancel_tray_menu, 0);
    }
}

#[repr(C)]
struct WndClassW {
    style: u32,
    lpfn_wnd_proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: isize,
    h_icon: isize,
    h_cursor: isize,
    hbr_background: isize,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

struct DeviceListenerState {
    proxy: EventLoopProxy<UserEvent>,
    last_refresh_ms: std::sync::atomic::AtomicI64,
}

static mut LISTENER_STATE: Option<DeviceListenerState> = None;

unsafe extern "system" fn wnd_proc(
    h_wnd: isize,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    if msg == WM_DEVICECHANGE {
        let event = w_param as u32;
        if event == DBT_DEVICEARRIVAL || event == DBT_DEVICEREMOVECOMPLETE {
            // 立刻打断主线程上的 TrackPopupMenu，避免热插拔卡死
            cancel_tray_popup_menus();

            if let Some(state) = (*std::ptr::addr_of!(LISTENER_STATE)).as_ref() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                let last = state
                    .last_refresh_ms
                    .load(std::sync::atomic::Ordering::Relaxed);
                // 尽快送达主线程；扫描侧还有 scan_worker 防抖
                if now - last > 100 {
                    state
                        .last_refresh_ms
                        .store(now, std::sync::atomic::Ordering::Relaxed);
                    let _ = state.proxy.send_event(UserEvent::DeviceHotplug);
                }
            }
            return 1;
        }
    }

    DefWindowProcW(h_wnd, msg, w_param, l_param)
}

const DEVICE_NOTIFY_WINDOW_HANDLE: u32 = 0x00000000;
const DEVICE_NOTIFY_ALL_INTERFACE_CLASSES: u32 = 0x00000004;

pub fn spawn_device_listener(proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || unsafe {
        LISTENER_STATE = Some(DeviceListenerState {
            proxy,
            last_refresh_ms: std::sync::atomic::AtomicI64::new(0),
        });

        let class_name: Vec<u16> = "MchoseDeviceListener\0".encode_utf16().collect();
        let module = GetModuleHandleW(std::ptr::null());

        let wnd_class = WndClassW {
            style: 0,
            lpfn_wnd_proc: wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: module,
            h_icon: 0,
            h_cursor: 0,
            hbr_background: 0,
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let h_wnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            module,
            std::ptr::null(),
        );

        if h_wnd == 0 {
            return;
        }

        let filter = DevBroadcastDeviceInterface {
            dbcc_size: std::mem::size_of::<DevBroadcastDeviceInterface>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
            dbcc_reserved: 0,
            dbcc_classguid: [0; 16],
            dbcc_name: [0; 1],
        };

        let notify_handle = RegisterDeviceNotificationW(
            h_wnd,
            &filter as *const _ as *const std::ffi::c_void,
            DEVICE_NOTIFY_WINDOW_HANDLE | DEVICE_NOTIFY_ALL_INTERFACE_CLASSES,
        );

        let mut msg = Msg {
            hwnd: 0,
            message: 0,
            w_param: 0,
            l_param: 0,
            time: 0,
            pt_x: 0,
            pt_y: 0,
        };

        loop {
            let ret = GetMessageW(&mut msg, 0, 0, 0);
            if ret <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if notify_handle != 0 {
            UnregisterDeviceNotification(notify_handle);
        }
        DestroyWindow(h_wnd);
        LISTENER_STATE = None;
    });
}
