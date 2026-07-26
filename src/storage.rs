//! Native Windows SSD temperatures via DeviceIoControl (NVMe SMART + ATA SMART).

use crate::model::DriveInfo;
use serde::Deserialize;
use std::mem::{size_of, zeroed};
use wmi::{COMLibrary, WMIConnection};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::IOCTL_STORAGE_QUERY_PROPERTY;

const SMART_RCV_DRIVE_DATA: u32 = 0x0007C088;

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_DiskDrive")]
#[serde(rename_all = "PascalCase")]
struct DiskDrive {
    index: Option<u32>,
    model: Option<String>,
    size: Option<u64>,
}

pub fn list_ssds() -> Vec<DriveInfo> {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    let disks: Vec<DiskDrive> = wmi.query().unwrap_or_default();

    let mut out = Vec::new();
    for d in disks {
        let Some(index) = d.index else { continue };
        let model = d
            .model
            .unwrap_or_else(|| format!("Disk {index}"))
            .trim()
            .to_string();
        let size_gb = d.size.unwrap_or(0) as f64 / (1024.0 * 1024.0 * 1024.0);
        if size_gb < 8.0 {
            continue;
        }
        out.push(DriveInfo {
            name: model,
            size_gb,
            temp_c: read_drive_temp(index),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn refresh_temps(drives: &mut [DriveInfo]) {
    // Re-read by matching Win32_DiskDrive model names to indices.
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let Ok(wmi) = WMIConnection::new(com) else {
        return;
    };
    let disks: Vec<DiskDrive> = wmi.query().unwrap_or_default();
    for drive in drives.iter_mut() {
        if let Some(d) = disks.iter().find(|d| {
            d.model
                .as_deref()
                .map(|m| m.trim() == drive.name)
                .unwrap_or(false)
        }) {
            if let Some(index) = d.index {
                drive.temp_c = read_drive_temp(index);
            }
        }
    }
}

fn read_drive_temp(index: u32) -> Option<f64> {
    read_nvme_smart_temp(index)
        .or_else(|| read_ata_smart_temp(index))
        .or_else(|| read_storage_temperature_property(index))
}

fn open_physical_drive(index: u32) -> Option<HANDLE> {
    let path = format!("\\\\.\\PhysicalDrive{index}");
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        )
        .ok()
        .or_else(|| {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
            .ok()
        })
    }
}

fn read_nvme_smart_temp(index: u32) -> Option<f64> {
    let handle = open_physical_drive(index)?;
    // Property IDs: StorageDeviceProtocolSpecificProperty=50, StorageAdapterProtocolSpecificProperty=49
    let result = unsafe {
        nvme_query(handle, 50, 0xFFFF_FFFF)
            .or_else(|| nvme_query(handle, 50, 0))
            .or_else(|| nvme_query(handle, 49, 0xFFFF_FFFF))
            .or_else(|| nvme_query(handle, 49, 0))
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

unsafe fn nvme_query(handle: HANDLE, property_id: u32, nsid: u32) -> Option<f64> {
    let query_base = 8usize;
    let proto_size = 40usize; // STORAGE_PROTOCOL_SPECIFIC_DATA
    let payload = 512usize;
    let total = query_base + proto_size + payload;
    let mut buf = vec![0u8; total];

    buf[0..4].copy_from_slice(&property_id.to_le_bytes());
    buf[4..8].copy_from_slice(&0u32.to_le_bytes()); // PropertyStandardQuery

    let mut off = query_base;
    // ProtocolTypeNvme = 3
    buf[off..off + 4].copy_from_slice(&3u32.to_le_bytes());
    off += 4;
    // NVMeDataTypeLogPage = 2
    buf[off..off + 4].copy_from_slice(&2u32.to_le_bytes());
    off += 4;
    // SMART/Health log page id = 2
    buf[off..off + 4].copy_from_slice(&2u32.to_le_bytes());
    off += 4;
    buf[off..off + 4].copy_from_slice(&nsid.to_le_bytes());
    off += 4;
    buf[off..off + 4].copy_from_slice(&(proto_size as u32).to_le_bytes());
    off += 4;
    buf[off..off + 4].copy_from_slice(&(payload as u32).to_le_bytes());

    let mut out = vec![0u8; total];
    let mut returned = 0u32;
    DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        Some(buf.as_ptr() as *const _),
        total as u32,
        Some(out.as_mut_ptr() as *mut _),
        total as u32,
        Some(&mut returned),
        None,
    )
    .ok()?;

    if returned < 48 {
        return None;
    }
    // Descriptor: Version, Size, then protocol specific starting at 8
    let data_offset = u32::from_le_bytes(out[8 + 16..8 + 20].try_into().ok()?) as usize;
    let data_len = u32::from_le_bytes(out[8 + 20..8 + 24].try_into().ok()?) as usize;
    let abs = 8 + data_offset;
    if data_len < 3 || out.len() < abs + 3 {
        return None;
    }
    let kelvin = u16::from_le_bytes([out[abs + 1], out[abs + 2]]);
    if !(273..=400).contains(&kelvin) {
        return None;
    }
    Some(kelvin as f64 - 273.0)
}

fn read_ata_smart_temp(index: u32) -> Option<f64> {
    let handle = open_physical_drive(index)?;
    let result = unsafe { ata_smart_temp(handle) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

#[repr(C)]
struct IdeRegs {
    features: u8,
    sector_count: u8,
    sector_number: u8,
    cyl_low: u8,
    cyl_high: u8,
    drive_head: u8,
    command: u8,
    reserved: u8,
}

#[repr(C)]
struct SendCmdInParams {
    buffer_size: u32,
    regs: IdeRegs,
    drive_number: u8,
    reserved: [u8; 3],
    reserved2: [u32; 4],
    buffer: [u8; 1],
}

#[repr(C)]
struct DriverStatus {
    driver_error: u8,
    ide_status: u8,
    reserved: [u8; 2],
    reserved2: [u32; 2],
}

#[repr(C)]
struct SendCmdOutParams {
    buffer_size: u32,
    driver_status: DriverStatus,
    buffer: [u8; 512],
}

unsafe fn ata_smart_temp(handle: HANDLE) -> Option<f64> {
    let mut input: SendCmdInParams = zeroed();
    input.buffer_size = 512;
    input.regs.features = 0xD0; // SMART READ DATA
    input.regs.sector_count = 1;
    input.regs.sector_number = 1;
    input.regs.cyl_low = 0x4F;
    input.regs.cyl_high = 0xC2;
    input.regs.drive_head = 0xA0;
    input.regs.command = 0xB0; // SMART
    input.drive_number = 0;

    let mut output: SendCmdOutParams = zeroed();
    let mut returned = 0u32;
    DeviceIoControl(
        handle,
        SMART_RCV_DRIVE_DATA,
        Some((&input as *const SendCmdInParams) as *const _),
        size_of::<SendCmdInParams>() as u32,
        Some((&mut output as *mut SendCmdOutParams) as *mut _),
        size_of::<SendCmdOutParams>() as u32,
        Some(&mut returned),
        None,
    )
    .ok()?;

    let data = &output.buffer;
    let mut best: Option<u8> = None;
    let mut i = 2usize;
    while i + 12 <= 362 {
        let id = data[i];
        let raw = data[i + 5];
        if id == 194 {
            return Some(raw as f64).filter(|_| (1..110).contains(&raw));
        }
        if id == 190 && (1..110).contains(&raw) {
            best = Some(raw);
        }
        i += 12;
    }
    best.map(|t| t as f64)
}

fn read_storage_temperature_property(index: u32) -> Option<f64> {
    let handle = open_physical_drive(index)?;
    let result = unsafe { storage_temp_property(handle) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

unsafe fn storage_temp_property(handle: HANDLE) -> Option<f64> {
    let mut query = [0u8; 8];
    query[0..4].copy_from_slice(&6u32.to_le_bytes()); // StorageDeviceTemperatureProperty
    let mut out = [0u8; 512];
    let mut returned = 0u32;
    DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        Some(query.as_ptr() as *const _),
        8,
        Some(out.as_mut_ptr() as *mut _),
        512,
        Some(&mut returned),
        None,
    )
    .ok()?;
    if returned < 24 {
        return None;
    }
    let info_count = u16::from_le_bytes(out[12..14].try_into().ok()?) as usize;
    let mut off = 24usize;
    for _ in 0..info_count.min(8) {
        if off + 4 > returned as usize {
            break;
        }
        let temperature = i16::from_le_bytes(out[off + 2..off + 4].try_into().ok()?);
        if (1..110).contains(&temperature) {
            return Some(temperature as f64);
        }
        off += 16;
    }
    None
}
