# System Monitor

Minimal Windows system monitor in Rust. One process, ~1 Hz refresh, no third-party sensor apps.

## Metrics

| Area | Fields | Source |
|------|--------|--------|
| CPU | name, clock | sysinfo |
| CPU | temp | ACPI thermal zone (if the board exposes it) |
| CPU | fan, power | Not available via normal Windows APIs on most DIY boards |
| GPU | name, clock, temp, power, VRAM, fan | NVIDIA NVML |
| RAM | used / total, slot inventory | sysinfo + WMI |
| RAM | per-slot temp | Not exposed without chipset/EC access |
| SSD | each drive + temp | Win32 `DeviceIoControl` (NVMe SMART / ATA SMART) |
| Network | up/down Mbps, ping | sysinfo + ICMP ping to 1.1.1.1 |

## Requirements

- Windows 10/11
- NVIDIA driver (for GPU metrics)

No HWiNFO / LibreHardwareMonitor required.

## Build & run

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo run --release
```

Release executable with embedded icon:

```powershell
cargo build --release
# output:
#   target\release\Pulse.exe
#   dist\Pulse.exe   (copy)
```

One-shot dump:

```powershell
cargo run --release -- --dump
```

Quit with `q` or `Esc`.

## Why some values show N/A

CPU package temperature, CPU fan RPM, CPU package power, and DIMM temperatures live behind motherboard Super-I/O / EC / SMBus registers. Windows does not publish those without a signed kernel helper (the path tools like HWiNFO use). This monitor stays user-mode and driver-free on purpose.

SSD temps and full NVIDIA GPU telemetry are read natively.
