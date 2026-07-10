#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChargeState {
    Unknown,
    NotCharging,
    Charging,
    Full,
}

impl ChargeState {
    pub fn label(&self) -> &'static str {
        match self {
            ChargeState::Unknown => "未知",
            ChargeState::NotCharging => "未充电",
            ChargeState::Charging => "充电中",
            ChargeState::Full => "已充满",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub percent: u8,
    pub charge_state: ChargeState,
}

#[derive(Debug, Clone)]
pub struct DpiInfo {
    pub value: u16,
    pub index: u8,
    pub count: u8,
}

impl DpiInfo {
    pub fn label(&self) -> String {
        format!(
            "DPI：{}（第{}档/共{}档）",
            self.value,
            self.index + 1,
            self.count
        )
    }
}

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub battery: BatteryInfo,
    pub dpi: Option<DpiInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceMode {
    Auto,
    Dark,
    Light,
}

impl AppearanceMode {
    pub fn label(self) -> &'static str {
        match self {
            AppearanceMode::Auto => "自动",
            AppearanceMode::Dark => "深色",
            AppearanceMode::Light => "浅色",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MchoseDevice {
    pub name: String,
    pub status: Option<DeviceStatus>,
}

impl MchoseDevice {
    pub fn charge_label(&self) -> &'static str {
        self.status
            .as_ref()
            .map(|status| status.battery.charge_state.label())
            .unwrap_or("未知")
    }

    pub fn dpi_label(&self) -> String {
        match &self.status {
            Some(status) if status.dpi.is_some() => status.dpi.as_ref().unwrap().label(),
            _ => "DPI：未知".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub devices: Vec<MchoseDevice>,
    pub current_index: usize,
    pub appearance_mode: AppearanceMode,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            current_index: 0,
            appearance_mode: AppearanceMode::Auto,
        }
    }

    pub fn set_devices(&mut self, devices: Vec<MchoseDevice>) {
        self.devices = devices;
        if self.devices.is_empty() {
            self.current_index = 0;
        } else if self.current_index >= self.devices.len() {
            self.current_index = 0;
        }
    }

    pub fn current_device(&self) -> Option<&MchoseDevice> {
        self.devices.get(self.current_index)
    }

    pub fn switch_next(&mut self) {
        if self.devices.is_empty() {
            self.current_index = 0;
            return;
        }
        self.current_index = (self.current_index + 1) % self.devices.len();
    }

    pub fn current_tooltip(&self) -> String {
        match self.current_device() {
            Some(device) => match &device.status {
                Some(status) => format!(
                    "{}：{}% · {}",
                    device.name,
                    status.battery.percent,
                    status.battery.charge_state.label()
                ),
                None => format!("{}：电量未知", device.name),
            },
            None => "未发现 MCHOSE 鼠标".to_string(),
        }
    }
}
