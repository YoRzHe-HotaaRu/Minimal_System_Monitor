use crate::model::{CpuInfo, MemoryInfo, RamSlot, Snapshot};
use serde::Deserialize;
use sysinfo::System;
use wmi::{COMLibrary, WMIConnection};

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_PhysicalMemory")]
#[serde(rename_all = "PascalCase")]
struct PhysicalMemory {
    bank_label: Option<String>,
    device_locator: Option<String>,
    capacity: Option<u64>,
    part_number: Option<String>,
    configured_clock_speed: Option<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename = "MSAcpi_ThermalZoneTemperature")]
#[serde(rename_all = "PascalCase")]
struct ThermalZone {
    instance_name: Option<String>,
    current_temperature: Option<u32>,
}

pub struct Collector {
    sys: System,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        Self { sys }
    }

    pub fn collect(&mut self, snap: &mut Snapshot) {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        snap.cpu.name = self
            .sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown CPU".into());

        snap.cpu.clock_mhz = if self.sys.cpus().is_empty() {
            None
        } else {
            let sum: u64 = self.sys.cpus().iter().map(|c| c.frequency()).sum();
            Some(sum as f64 / self.sys.cpus().len() as f64)
        };

        snap.cpu.usage_pct = if self.sys.cpus().is_empty() {
            None
        } else {
            let sum: f32 = self.sys.cpus().iter().map(|c| c.cpu_usage()).sum();
            Some((sum / self.sys.cpus().len() as f32) as f64)
        };

        snap.memory.used_gb = self.sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        snap.memory.total_gb = self.sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        snap.memory.slots = list_ram_slots();

        // Best-effort ACPI thermal zone (often absent on DIY desktops).
        if snap.cpu.temp_c.is_none() {
            snap.cpu.temp_c = read_acpi_thermal();
        }

        snap.note = native_note(&snap.cpu, &snap.memory);
    }
}

fn list_ram_slots() -> Vec<RamSlot> {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    let rows: Vec<PhysicalMemory> = wmi.query().unwrap_or_default();
    let mut slots = Vec::new();
    for row in rows {
        let locator = row.device_locator.unwrap_or_default();
        let bank = row.bank_label.unwrap_or_default();
        let part = row.part_number.unwrap_or_default().trim().to_string();
        let gb = row.capacity.unwrap_or(0) as f64 / (1024.0 * 1024.0 * 1024.0);
        let speed = row
            .configured_clock_speed
            .map(|s| format!("{s} MT/s"))
            .unwrap_or_else(|| "?".into());
        let label = if !locator.is_empty() {
            format!("{locator} {gb:.0}GB {speed}")
        } else if !bank.is_empty() {
            format!("{bank} {gb:.0}GB {speed}")
        } else if !part.is_empty() {
            format!("{part} {gb:.0}GB")
        } else {
            format!("DIMM {gb:.0}GB")
        };
        slots.push(RamSlot {
            label,
            // DIMM thermal sensors are not exposed by Windows without a chipset driver.
            temp_c: None,
        });
    }
    slots
}

fn read_acpi_thermal() -> Option<f64> {
    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::with_namespace_path(r"root\wmi", com).ok()?;
    let zones: Vec<ThermalZone> = wmi.query().ok()?;
    let mut best: Option<f64> = None;
    for z in zones {
        let Some(raw) = z.current_temperature else {
            continue;
        };
        // Tenths of Kelvin
        let c = (raw as f64 / 10.0) - 273.15;
        if !(10.0..110.0).contains(&c) {
            continue;
        }
        let name = z.instance_name.unwrap_or_default().to_ascii_lowercase();
        // Prefer CPU-ish zones when present
        if name.contains("cpu") || name.contains("tzcpu") || name.contains("processor") {
            return Some(c);
        }
        best = Some(c);
    }
    best
}

fn native_note(cpu: &CpuInfo, memory: &MemoryInfo) -> String {
    let mut missing = Vec::new();
    if cpu.temp_c.is_none() {
        missing.push("CPU temp");
    }
    if cpu.fan_rpm.is_none() {
        missing.push("CPU fan");
    }
    if cpu.power_w.is_none() {
        missing.push("CPU power");
    }
    if memory.slots.iter().all(|s| s.temp_c.is_none()) {
        missing.push("RAM temp");
    }
    if missing.is_empty() {
        "Native sensors OK.".into()
    } else {
        format!(
            "{} unavailable via Windows APIs (needs chipset/EC access).",
            missing.join(", ")
        )
    }
}
