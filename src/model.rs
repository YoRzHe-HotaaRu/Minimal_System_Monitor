#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub cpu: CpuInfo,
    pub gpu: GpuInfo,
    pub memory: MemoryInfo,
    pub drives: Vec<DriveInfo>,
    pub network: NetworkInfo,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct CpuInfo {
    pub name: String,
    pub clock_mhz: Option<f64>,
    pub usage_pct: Option<f64>,
    pub temp_c: Option<f64>,
    pub fan_rpm: Option<f64>,
    pub power_w: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub name: String,
    pub clock_mhz: Option<f64>,
    pub usage_pct: Option<f64>,
    pub temp_c: Option<f64>,
    pub power_w: Option<f64>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub fan_pct: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub used_gb: f64,
    pub total_gb: f64,
    pub slots: Vec<RamSlot>,
}

#[derive(Debug, Clone, Default)]
pub struct RamSlot {
    pub label: String,
    pub temp_c: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct DriveInfo {
    pub name: String,
    pub size_gb: f64,
    pub temp_c: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkInfo {
    pub adapter: String,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: Option<f64>,
    pub ping_target: String,
}
