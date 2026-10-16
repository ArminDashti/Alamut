//! Static hardware inventory (CPU, RAM, disks, GPU, motherboard).

use alamut_core::types::{
    CpuInfo, DiskInfo, DiskKind, GpuInfo, HardwareInventory, MemoryInfo, MotherboardInfo, OsInfo,
};
use sysinfo::System;
use tracing::warn;

/// Collect hardware inventory. On Windows uses WMI; elsewhere returns empty stubs.
pub fn collect() -> HardwareInventory {
    let mut inv = collect_hardware();
    inv.os = collect_os();
    inv
}

fn collect_os() -> OsInfo {
    OsInfo {
        name: System::name().unwrap_or_default(),
        version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        architecture: std::env::consts::ARCH.to_string(),
    }
}

fn collect_hardware() -> HardwareInventory {
    #[cfg(windows)]
    {
        match win::collect_wmi() {
            Ok(inv) => return inv,
            Err(e) => warn!("hardware inventory WMI failed: {e:#}"),
        }
    }
    fallback_empty()
}

fn fallback_empty() -> HardwareInventory {
    HardwareInventory {
        cpu: CpuInfo {
            brand: String::new(),
            model: String::new(),
            base_speed_mhz: None,
            physical_cores: None,
            logical_processors: 0,
        },
        memory: MemoryInfo {
            brand: String::new(),
            model: String::new(),
            size_bytes: 0,
            speed_mhz: None,
            modules: 0,
            module_size_bytes: None,
            memory_type: String::new(),
            form_factor: String::new(),
            configured_speed_mhz: None,
        },
        disks: Vec::new(),
        gpus: Vec::new(),
        motherboard: MotherboardInfo {
            brand: String::new(),
            model: String::new(),
            ram_slots: None,
        },
        os: OsInfo::default(),
    }
}

/// Best-effort CUDA / Tensor core counts from consumer GPU model names.
fn estimate_nvidia_cores(name: &str) -> (Option<u32>, Option<u32>) {
    let lower = name.to_lowercase();
    if !lower.contains("nvidia") && !lower.contains("geforce") && !lower.contains("rtx") && !lower.contains("gtx") {
        return (None, None);
    }
    // Rough public specs for common cards; unknown models return None.
    let table: &[(&str, u32, u32)] = &[
        ("rtx 4090", 16384, 512),
        ("rtx 4080", 9728, 304),
        ("rtx 4070 ti", 7680, 240),
        ("rtx 4070", 5888, 184),
        ("rtx 4060 ti", 4352, 136),
        ("rtx 4060", 3072, 96),
        ("rtx 3090", 10496, 328),
        ("rtx 3080", 8704, 272),
        ("rtx 3070", 5888, 184),
        ("rtx 3060 ti", 4864, 152),
        ("rtx 3060", 3584, 112),
        ("rtx 2080 ti", 4352, 544),
        ("rtx 2080", 2944, 368),
        ("rtx 2070", 2304, 288),
        ("rtx 2060", 1920, 240),
    ];
    for (pat, cuda, tensor) in table {
        if lower.contains(pat) {
            return (Some(*cuda), Some(*tensor));
        }
    }
    (None, None)
}

fn split_brand_model(full: &str, known_brands: &[&str]) -> (String, String) {
    let trimmed = full.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new());
    }
    let lower = trimmed.to_lowercase();
    // Prefer longest brand match so "WDC" wins over "WD", etc.
    let mut best: Option<&str> = None;
    for brand in known_brands {
        let b = brand.to_lowercase();
        let hits = lower.starts_with(&b)
            && (lower.len() == b.len()
                || lower.as_bytes().get(b.len()).is_some_and(|c| !c.is_ascii_alphanumeric()));
        let contains_word = lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|p| p == b);
        if hits || (lower.starts_with(&b) && b.len() >= 3) || contains_word {
            if best.map(|cur| brand.len() > cur.len()).unwrap_or(true) {
                best = Some(*brand);
            }
        }
    }
    if let Some(brand) = best {
        let b = brand.to_lowercase();
        if lower.starts_with(&b) {
            let rest = trimmed[brand.len()..]
                .trim()
                .trim_start_matches([' ', '-', '_']);
            return (brand.to_string(), if rest.is_empty() { trimmed.to_string() } else { rest.to_string() });
        }
        return (brand.to_string(), trimmed.to_string());
    }
    // First token as brand, rest as model
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let brand = parts.next().unwrap_or("").to_string();
    let model = parts.next().unwrap_or("").to_string();
    if model.is_empty() {
        (brand.clone(), brand)
    } else {
        (brand, model)
    }
}

fn disk_kind_from_model(model: &str, media_type: Option<&str>) -> DiskKind {
    let m = model.to_lowercase();
    let media = media_type.unwrap_or("").to_lowercase();
    if m.contains("ssd")
        || m.contains("nvme")
        || m.contains("solid")
        || media.contains("ssd")
        || media.contains("solid")
    {
        DiskKind::Ssd
    } else {
        DiskKind::Hdd
    }
}

fn parse_disk_brand_model(model: &str) -> (String, String) {
    const BRANDS: &[&str] = &[
        "Western Digital",
        "Samsung",
        "Seagate",
        "Crucial",
        "Micron",
        "Kingston",
        "SanDisk",
        "SK hynix",
        "Hynix",
        "Toshiba",
        "Kioxia",
        "Hitachi",
        "HGST",
        "Apple",
        "Corsair",
        "ADATA",
        "Lexar",
        "Team",
        "PNY",
        "Intel",
        "WDC",
        "WD",
    ];
    split_brand_model(model, BRANDS)
}

fn smbios_memory_type(code: Option<u16>) -> String {
    match code {
        Some(20) => "DDR".into(),
        Some(21) => "DDR2".into(),
        Some(22) => "DDR2 FB-DIMM".into(),
        Some(24) => "DDR3".into(),
        Some(26) => "DDR4".into(),
        Some(34) => "DDR5".into(),
        Some(18) => "DDR".into(),
        Some(19) => "DDR2".into(),
        _ => String::new(),
    }
}

fn memory_form_factor(code: Option<u16>) -> String {
    match code {
        Some(8) => "DIMM".into(),
        Some(12) => "SODIMM".into(),
        Some(9) => "TSOP".into(),
        Some(13) => "SRIMM".into(),
        Some(10) => "Row of chips".into(),
        _ => String::new(),
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use anyhow::{anyhow, Context, Result};
    use serde::Deserialize;
    use wmi::{COMLibrary, WMIConnection};

    #[derive(Deserialize, Debug, Default)]
    struct Win32Processor {
        #[serde(rename = "Name")]
        name: Option<String>,
        #[serde(rename = "Manufacturer")]
        manufacturer: Option<String>,
        #[serde(rename = "MaxClockSpeed")]
        max_clock_speed: Option<u32>,
        #[serde(rename = "NumberOfCores")]
        number_of_cores: Option<u32>,
        #[serde(rename = "NumberOfLogicalProcessors")]
        number_of_logical_processors: Option<u32>,
    }

    #[derive(Deserialize, Debug, Default)]
    struct Win32PhysicalMemory {
        #[serde(rename = "Manufacturer")]
        manufacturer: Option<String>,
        #[serde(rename = "PartNumber")]
        part_number: Option<String>,
        #[serde(rename = "Capacity")]
        capacity: Option<u64>,
        #[serde(rename = "Speed")]
        speed: Option<u32>,
        #[serde(rename = "ConfiguredClockSpeed")]
        configured_clock_speed: Option<u32>,
        #[serde(rename = "FormFactor")]
        form_factor: Option<u16>,
        #[serde(rename = "SMBIOSMemoryType")]
        smbios_memory_type: Option<u16>,
    }

    #[derive(Deserialize, Debug, Default)]
    struct Win32DiskDrive {
        #[serde(rename = "Model")]
        model: Option<String>,
        #[serde(rename = "Size")]
        size: Option<u64>,
        #[serde(rename = "MediaType")]
        media_type: Option<String>,
        #[serde(rename = "InterfaceType")]
        interface_type: Option<String>,
        #[serde(rename = "SerialNumber")]
        serial_number: Option<String>,
        #[serde(rename = "Partitions")]
        partitions: Option<u32>,
        #[serde(rename = "FirmwareRevision")]
        firmware_revision: Option<String>,
    }

    #[derive(Deserialize, Debug, Default)]
    struct Win32VideoController {
        #[serde(rename = "Name")]
        name: Option<String>,
        #[serde(rename = "AdapterRAM")]
        adapter_ram: Option<u32>,
        #[serde(rename = "AdapterCompatibility")]
        adapter_compatibility: Option<String>,
    }

    #[derive(Deserialize, Debug, Default)]
    struct Win32BaseBoard {
        #[serde(rename = "Manufacturer")]
        manufacturer: Option<String>,
        #[serde(rename = "Product")]
        product: Option<String>,
    }

    #[derive(Deserialize, Debug, Default)]
    struct Win32PhysicalMemoryArray {
        #[serde(rename = "MemoryDevices")]
        memory_devices: Option<u32>,
    }

    pub fn collect_wmi() -> Result<HardwareInventory> {
        let com = COMLibrary::new().context("COM init")?;
        let wmi = WMIConnection::new(com).context("WMI connect")?;

        let cpu = collect_cpu(&wmi)?;
        let memory = collect_memory(&wmi)?;
        let disks = collect_disks(&wmi)?;
        let gpus = collect_gpus(&wmi)?;
        let motherboard = collect_motherboard(&wmi)?;

        Ok(HardwareInventory {
            cpu,
            memory,
            disks,
            gpus,
            motherboard,
            os: OsInfo::default(),
        })
    }

    fn collect_cpu(wmi: &WMIConnection) -> Result<CpuInfo> {
        let rows: Vec<Win32Processor> = wmi.raw_query("SELECT Name, Manufacturer, MaxClockSpeed, NumberOfCores, NumberOfLogicalProcessors FROM Win32_Processor")?;
        let row = rows.into_iter().next().ok_or_else(|| anyhow!("no Win32_Processor"))?;
        let name = row.name.unwrap_or_default();
        let manufacturer = row.manufacturer.unwrap_or_default();
        let brands = ["Intel", "AMD", "Apple", "Qualcomm", "ARM", "Microsoft"];
        let (brand, model) = if !manufacturer.is_empty() {
            let brand = brands
                .iter()
                .find(|b| manufacturer.to_lowercase().contains(&b.to_lowercase()))
                .copied()
                .unwrap_or(manufacturer.as_str())
                .to_string();
            (brand, name)
        } else {
            split_brand_model(&name, &brands)
        };
        Ok(CpuInfo {
            brand,
            model,
            base_speed_mhz: row.max_clock_speed.map(|s| s as u64),
            physical_cores: row.number_of_cores,
            logical_processors: row.number_of_logical_processors.unwrap_or(0),
        })
    }

    fn collect_memory(wmi: &WMIConnection) -> Result<MemoryInfo> {
        let rows: Vec<Win32PhysicalMemory> = wmi.raw_query(
            "SELECT Manufacturer, PartNumber, Capacity, Speed, ConfiguredClockSpeed, FormFactor, SMBIOSMemoryType FROM Win32_PhysicalMemory",
        )?;
        if rows.is_empty() {
            return Ok(MemoryInfo {
                brand: String::new(),
                model: String::new(),
                size_bytes: 0,
                speed_mhz: None,
                modules: 0,
                module_size_bytes: None,
                memory_type: String::new(),
                form_factor: String::new(),
                configured_speed_mhz: None,
            });
        }
        let mut total = 0u64;
        let mut rated_speed = None;
        let mut configured_speed = None;
        let mut brand = String::new();
        let mut model = String::new();
        let mut memory_type = String::new();
        let mut form_factor = String::new();
        let mut sizes: Vec<u64> = Vec::new();
        for row in &rows {
            let cap = row.capacity.unwrap_or(0);
            total += cap;
            if cap > 0 {
                sizes.push(cap);
            }
            if rated_speed.is_none() {
                rated_speed = row.speed;
            } else if let (Some(a), Some(b)) = (rated_speed, row.speed) {
                rated_speed = Some(a.max(b));
            }
            if configured_speed.is_none() {
                configured_speed = row.configured_clock_speed;
            } else if let (Some(a), Some(b)) = (configured_speed, row.configured_clock_speed) {
                configured_speed = Some(a.max(b));
            }
            if brand.is_empty() {
                brand = row
                    .manufacturer
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
            if model.is_empty() {
                model = row.part_number.as_deref().unwrap_or("").trim().to_string();
            }
            if memory_type.is_empty() {
                memory_type = smbios_memory_type(row.smbios_memory_type);
            }
            if form_factor.is_empty() {
                form_factor = memory_form_factor(row.form_factor);
            }
        }
        let module_size_bytes = if !sizes.is_empty() && sizes.iter().all(|&s| s == sizes[0]) {
            Some(sizes[0])
        } else {
            None
        };
        // Rated vs configured clocks (UI can show both).
        let speed_mhz = rated_speed;
        let configured_speed_mhz = configured_speed;
        // Normalize common OEM memory brand strings
        let brand_norm = if brand.is_empty() || brand.eq_ignore_ascii_case("unknown") {
            String::new()
        } else {
            brand
        };
        Ok(MemoryInfo {
            brand: brand_norm,
            model,
            size_bytes: total,
            speed_mhz,
            modules: rows.len() as u32,
            module_size_bytes,
            memory_type,
            form_factor,
            configured_speed_mhz,
        })
    }

    fn collect_disks(wmi: &WMIConnection) -> Result<Vec<DiskInfo>> {
        let rows: Vec<Win32DiskDrive> = wmi.raw_query(
            "SELECT Model, Size, MediaType, InterfaceType, SerialNumber, Partitions, FirmwareRevision FROM Win32_DiskDrive",
        )?;
        let mut out = Vec::new();
        for row in rows {
            let full_model = row.model.unwrap_or_default().trim().to_string();
            if full_model.is_empty() {
                continue;
            }
            // Skip virtual / empty size disks
            let capacity = row.size.unwrap_or(0);
            if capacity == 0 {
                continue;
            }
            let lower = full_model.to_lowercase();
            if lower.contains("virtual") || lower.contains("vbox") || lower.contains("qemu") {
                continue;
            }
            let (brand, model) = parse_disk_brand_model(&full_model);
            let mut kind = disk_kind_from_model(
                &full_model,
                row.media_type.as_deref(),
            );
            let interface_type = row.interface_type.clone().unwrap_or_default();
            if interface_type.eq_ignore_ascii_case("SCSI") && full_model.to_lowercase().contains("nvme")
            {
                kind = DiskKind::Ssd;
            }
            if interface_type.to_lowercase().contains("nvme") {
                kind = DiskKind::Ssd;
            }
            out.push(DiskInfo {
                brand: if brand.is_empty() {
                    model.clone()
                } else {
                    brand
                },
                model: if model.is_empty() {
                    full_model
                } else {
                    model
                },
                capacity_bytes: capacity,
                kind,
                interface_type,
                media_type: row.media_type.unwrap_or_default().trim().to_string(),
                serial_number: row
                    .serial_number
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                partitions: row.partitions,
                firmware_revision: row
                    .firmware_revision
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            });
        }
        Ok(out)
    }

    fn collect_gpus(wmi: &WMIConnection) -> Result<Vec<GpuInfo>> {
        let rows: Vec<Win32VideoController> =
            wmi.raw_query("SELECT Name, AdapterRAM, AdapterCompatibility FROM Win32_VideoController")?;
        let mut out = Vec::new();
        for row in rows {
            let name = row.name.unwrap_or_default().trim().to_string();
            if name.is_empty() {
                continue;
            }
            let lower = name.to_lowercase();
            // Skip basic Microsoft display adapters
            if lower.contains("microsoft basic") || lower.contains("remote desktop") {
                continue;
            }
            let brands = [
                "NVIDIA", "AMD", "Intel", "Apple", "Qualcomm", "Matrox", "ASUS", "Gigabyte",
            ];
            let (brand, model) = if let Some(compat) = row.adapter_compatibility.as_deref() {
                let brand = brands
                    .iter()
                    .find(|b| compat.to_lowercase().contains(&b.to_lowercase()))
                    .or_else(|| {
                        brands
                            .iter()
                            .find(|b| lower.contains(&b.to_lowercase()))
                    })
                    .copied()
                    .unwrap_or("Unknown")
                    .to_string();
                (brand, name.clone())
            } else {
                split_brand_model(&name, &brands)
            };
            // AdapterRAM is often u32 and caps at ~4GB; treat 0xFFFFFFFF / weird values carefully
            let memory_bytes = row.adapter_ram.and_then(|r| {
                if r == 0 || r == u32::MAX {
                    None
                } else {
                    Some(r as u64)
                }
            });
            let (cuda, tensor) = estimate_nvidia_cores(&name);
            out.push(GpuInfo {
                brand,
                model,
                memory_bytes,
                cuda_cores: cuda,
                tensor_cores: tensor,
            });
        }
        Ok(out)
    }

    fn collect_motherboard(wmi: &WMIConnection) -> Result<MotherboardInfo> {
        let boards: Vec<Win32BaseBoard> =
            wmi.raw_query("SELECT Manufacturer, Product FROM Win32_BaseBoard")?;
        let board = boards.into_iter().next().unwrap_or_default();
        let brand = board.manufacturer.unwrap_or_default().trim().to_string();
        let model = board.product.unwrap_or_default().trim().to_string();

        let arrays: Vec<Win32PhysicalMemoryArray> =
            wmi.raw_query("SELECT MemoryDevices FROM Win32_PhysicalMemoryArray").unwrap_or_default();
        let ram_slots = arrays
            .into_iter()
            .filter_map(|a| a.memory_devices)
            .max();

        Ok(MotherboardInfo {
            brand,
            model,
            ram_slots,
        })
    }
}
