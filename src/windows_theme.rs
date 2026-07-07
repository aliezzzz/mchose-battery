#[cfg(windows)]
pub fn apply_menu_appearance(appearance: crate::app_state::AppearanceMode) {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    const PREFERRED_APP_MODE_DEFAULT: i32 = 0;
    const PREFERRED_APP_MODE_ALLOW_DARK: i32 = 1;
    const PREFERRED_APP_MODE_FORCE_DARK: i32 = 2;
    const PREFERRED_APP_MODE_FORCE_LIGHT: i32 = 3;

    let mode = match appearance {
        crate::app_state::AppearanceMode::Auto => PREFERRED_APP_MODE_ALLOW_DARK,
        crate::app_state::AppearanceMode::Dark => PREFERRED_APP_MODE_FORCE_DARK,
        crate::app_state::AppearanceMode::Light => PREFERRED_APP_MODE_FORCE_LIGHT,
    };

    unsafe {
        let module = LoadLibraryA(c"uxtheme.dll".as_ptr().cast());
        if module.is_null() {
            return;
        }

        let set_preferred_app_mode = GetProcAddress(module, 135 as _);
        if let Some(proc) = set_preferred_app_mode {
            let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                std::mem::transmute(proc);
            set_preferred_app_mode(mode);
        } else {
            let _ = PREFERRED_APP_MODE_DEFAULT;
            return;
        }

        let flush_menu_themes = GetProcAddress(module, 136 as _);
        if let Some(proc) = flush_menu_themes {
            let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(proc);
            flush_menu_themes();
        }
    }
}

#[cfg(not(windows))]
pub fn apply_menu_appearance(_appearance: crate::app_state::AppearanceMode) {}
