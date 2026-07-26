//! Telemetry history for sparklines / animated gauges.

const CAP: usize = 72;

#[derive(Clone, Default)]
pub struct Series {
    data: Vec<u64>,
}

impl Series {
    pub fn push(&mut self, value: f64) {
        let v = if value.is_finite() && value >= 0.0 {
            (value * 100.0).round().max(0.0) as u64 // store centi-units for finer sparkles
        } else {
            0
        };
        self.data.push(v);
        if self.data.len() > CAP {
            self.data.remove(0);
        }
    }

    pub fn push_raw(&mut self, value: u64) {
        self.data.push(value);
        if self.data.len() > CAP {
            self.data.remove(0);
        }
    }

    /// Min-max normalize into 1..=100 so flat signals still show shape.
    pub fn normalized(&self) -> Vec<u64> {
        if self.data.is_empty() {
            return vec![1; 8];
        }
        let min = *self.data.iter().min().unwrap_or(&0);
        let max = *self.data.iter().max().unwrap_or(&1);
        let span = max.saturating_sub(min).max(1);
        // If almost flat, amplify tiny noise / draw a gentle baseline wave
        if span <= 2 {
            return self
                .data
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let base = 35 + ((*v).min(100) as usize % 20);
                    let wobble = (i % 7) as u64;
                    (base as u64 + wobble).min(100).max(1)
                })
                .collect();
        }
        self.data
            .iter()
            .map(|v| {
                let n = ((*v - min) * 99) / span + 1;
                n.min(100).max(1)
            })
            .collect()
    }
}

#[derive(Clone, Default)]
pub struct History {
    pub cpu_clock: Series,
    pub cpu_usage: Series,
    pub gpu_temp: Series,
    pub gpu_clock: Series,
    pub gpu_power: Series,
    pub gpu_usage: Series,
    pub ram_pct: Series,
    pub net_down: Series,
    pub net_up: Series,
    pub tick: u64,
}

impl History {
    pub fn push_from_snapshot(&mut self, snap: &crate::model::Snapshot) {
        self.tick = self.tick.wrapping_add(1);
        self.cpu_clock
            .push(snap.cpu.clock_mhz.unwrap_or(0.0));
        self.cpu_usage
            .push(snap.cpu.usage_pct.unwrap_or(0.0));
        self.gpu_temp.push(snap.gpu.temp_c.unwrap_or(0.0));
        self.gpu_clock
            .push(snap.gpu.clock_mhz.unwrap_or(0.0));
        self.gpu_power.push(snap.gpu.power_w.unwrap_or(0.0));
        self.gpu_usage
            .push(snap.gpu.usage_pct.unwrap_or(0.0));
        let ram_pct = if snap.memory.total_gb > 0.0 {
            (snap.memory.used_gb / snap.memory.total_gb) * 100.0
        } else {
            0.0
        };
        self.ram_pct.push(ram_pct);
        self.net_down.push(snap.network.download_mbps);
        self.net_up.push(snap.network.upload_mbps);
    }
}
