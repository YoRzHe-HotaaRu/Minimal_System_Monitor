use crate::model::NetworkInfo;
use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::Networks;

const PING_TARGET: &str = "1.1.1.1";
const PING_EVERY: Duration = Duration::from_secs(5);

pub struct NetworkCollector {
    networks: Networks,
    last: Instant,
    last_rx: u64,
    last_tx: u64,
    adapter: String,
    last_ping: Instant,
    ping_ms: Option<f64>,
}

impl NetworkCollector {
    pub fn new() -> Self {
        let mut networks = Networks::new_with_refreshed_list();
        let (adapter, rx, tx) = pick_adapter(&networks);
        // Prime counters so first delta isn't a huge spike from boot totals.
        networks.refresh(true);
        Self {
            networks,
            last: Instant::now(),
            last_rx: rx,
            last_tx: tx,
            adapter,
            last_ping: Instant::now()
                .checked_sub(PING_EVERY)
                .unwrap_or_else(Instant::now),
            ping_ms: None,
        }
    }

    pub fn collect(&mut self, net: &mut NetworkInfo) {
        self.networks.refresh(true);
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().max(0.001);

        let (name, rx, tx) = {
            if let Some(data) = self.networks.iter().find(|(n, _)| **n == self.adapter) {
                (self.adapter.clone(), data.1.total_received(), data.1.total_transmitted())
            } else {
                let picked = pick_adapter(&self.networks);
                self.adapter = picked.0.clone();
                picked
            }
        };

        let down_bps = rx.saturating_sub(self.last_rx) as f64 / dt;
        let up_bps = tx.saturating_sub(self.last_tx) as f64 / dt;

        net.adapter = name;
        net.download_mbps = down_bps * 8.0 / 1_000_000.0;
        net.upload_mbps = up_bps * 8.0 / 1_000_000.0;
        net.ping_target = PING_TARGET.to_string();

        self.last_rx = rx;
        self.last_tx = tx;
        self.last = now;

        if now.duration_since(self.last_ping) >= PING_EVERY {
            self.ping_ms = icmp_ping(PING_TARGET);
            self.last_ping = now;
        }
        net.ping_ms = self.ping_ms;
    }
}

fn pick_adapter(networks: &Networks) -> (String, u64, u64) {
    let mut best: Option<(String, u64, u64, u64)> = None;
    for (name, data) in networks.iter() {
        let lname = name.to_ascii_lowercase();
        if lname.contains("loopback") || lname.contains("vethernet") || lname.contains("hyper-v")
        {
            continue;
        }
        let score = data.total_received() + data.total_transmitted();
        let wifi_boost = if lname.contains("wi-fi")
            || lname.contains("wifi")
            || lname.contains("wlan")
            || lname.contains("wireless")
        {
            1u64 << 62
        } else {
            0
        };
        let ranked = score + wifi_boost;
        if best.as_ref().map(|b| b.3).unwrap_or(0) < ranked {
            best = Some((
                name.to_string(),
                data.total_received(),
                data.total_transmitted(),
                ranked,
            ));
        }
    }
    best.map(|(n, r, t, _)| (n, r, t))
        .unwrap_or_else(|| ("N/A".into(), 0, 0))
}

fn icmp_ping(host: &str) -> Option<f64> {
    // Lightweight: one ICMP echo via Windows ping. Avoids raw socket admin requirement.
    let start = Instant::now();
    let output = Command::new("ping")
        .args(["-n", "1", "-w", "1000", host])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // English: "time=12ms" / "time<1ms" ; also "Average = 12ms"
    for token in stdout.split_whitespace() {
        let t = token.to_ascii_lowercase();
        if let Some(rest) = t.strip_prefix("time=") {
            let num: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(v) = num.parse::<f64>() {
                return Some(v);
            }
        }
        if t.starts_with("time<") {
            return Some(1.0);
        }
    }
    let _ = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
    Some(start.elapsed().as_secs_f64() * 1000.0)
}
