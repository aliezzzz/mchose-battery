use anyhow::Result;
use windows_registry::CURRENT_USER;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "MchoseBatteryTray";

pub fn is_enabled() -> Result<bool> {
    let expected = startup_command()?;
    let Ok(key) = CURRENT_USER.open(RUN_KEY) else {
        return Ok(false);
    };

    Ok(key
        .get_string(VALUE_NAME)
        .map(|value| value == expected)
        .unwrap_or(false))
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled {
        CURRENT_USER
            .create(RUN_KEY)?
            .set_string(VALUE_NAME, startup_command()?)?;
    } else if let Ok(key) = CURRENT_USER.options().read().write().open(RUN_KEY) {
        // If the value is already absent, startup is already disabled.
        if key.get_value(VALUE_NAME).is_ok() {
            key.remove_value(VALUE_NAME)?;
        }
    }

    Ok(())
}

fn startup_command() -> Result<String> {
    Ok(format!("\"{}\"", std::env::current_exe()?.display()))
}
