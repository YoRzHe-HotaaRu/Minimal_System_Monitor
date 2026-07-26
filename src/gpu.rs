use crate::model::GpuInfo;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;

pub struct GpuCollector {
    nvml: Option<Nvml>,
    init_error: Option<String>,
}

impl GpuCollector {
    pub fn new() -> Self {
        match Nvml::init() {
            Ok(nvml) => Self {
                nvml: Some(nvml),
                init_error: None,
            },
            Err(e) => Self {
                nvml: None,
                init_error: Some(format!("NVML init failed: {e}")),
            },
        }
    }

    pub fn collect(&self, gpu: &mut GpuInfo) {
        let Some(nvml) = &self.nvml else {
            gpu.error = self.init_error.clone();
            return;
        };

        // Prefer discrete NVIDIA (skip integrated if multiple; index 0 is usually dGPU on laptops,
        // on desktops with AMD iGPU + NVIDIA dGPU NVML only sees NVIDIA devices).
        let device = match nvml.device_by_index(0) {
            Ok(d) => d,
            Err(e) => {
                gpu.error = Some(format!("GPU open failed: {e}"));
                return;
            }
        };

        gpu.error = None;
        gpu.name = device.name().unwrap_or_else(|_| "NVIDIA GPU".into());

        if let Ok(c) = device.clock_info(Clock::Graphics) {
            gpu.clock_mhz = Some(c as f64);
        }
        if let Ok(t) = device.temperature(TemperatureSensor::Gpu) {
            gpu.temp_c = Some(t as f64);
        }
        if let Ok(mw) = device.power_usage() {
            gpu.power_w = Some(mw as f64 / 1000.0);
        }
        if let Ok(mem) = device.memory_info() {
            gpu.vram_used_mb = Some(mem.used / (1024 * 1024));
            gpu.vram_total_mb = Some(mem.total / (1024 * 1024));
        }
        if let Ok(fan) = device.fan_speed(0) {
            gpu.fan_pct = Some(fan);
        }
        if let Ok(util) = device.utilization_rates() {
            gpu.usage_pct = Some(util.gpu as f64);
        }
    }
}
