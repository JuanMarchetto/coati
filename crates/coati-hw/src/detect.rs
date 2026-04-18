use sysinfo::{Disks, System};

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub ram_total_bytes: u64,
    pub ram_available_bytes: u64,
    pub cpu_cores: usize,
    pub cpu_model: String,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub gpus: Vec<GpuInfo>,
    pub disk_free_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub vendor: String,
    pub name: String,
    pub vram_bytes: u64,
}

pub fn detect() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    let flags = read_cpu_flags();

    HardwareInfo {
        ram_total_bytes: sys.total_memory(),
        ram_available_bytes: sys.available_memory(),
        cpu_cores: sys.cpus().len(),
        cpu_model,
        has_avx2: flags.contains("avx2"),
        has_avx512: flags.contains("avx512f"),
        gpus: detect_gpus(),
        disk_free_bytes: disk_free_home(),
    }
}

fn disk_free_home() -> u64 {
    let disks = Disks::new_with_refreshed_list();
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    // prefer the disk mounted at $HOME, fall back to /
    disks
        .iter()
        .find(|d| home.starts_with(d.mount_point()))
        .map(|d| d.available_space())
        .unwrap_or_else(|| {
            disks
                .iter()
                .find(|d| d.mount_point() == std::path::Path::new("/"))
                .map(|d| d.available_space())
                .unwrap_or(0)
        })
}

#[cfg(target_os = "linux")]
fn read_cpu_flags() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("flags"))
        .unwrap_or("")
        .to_string()
}

#[cfg(not(target_os = "linux"))]
fn read_cpu_flags() -> String {
    String::new()
}

fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // NVML (Nvidia)
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        if let Ok(count) = nvml.device_count() {
            for i in 0..count {
                if let Ok(dev) = nvml.device_by_index(i) {
                    let name = dev.name().unwrap_or_default();
                    let mem = dev.memory_info().map(|m| m.total).unwrap_or(0);
                    gpus.push(GpuInfo {
                        vendor: "NVIDIA".into(),
                        name,
                        vram_bytes: mem,
                    });
                }
            }
        }
    }

    // AMD fallback (rocm-smi); best-effort, silent failure
    if gpus.is_empty() {
        if let Ok(out) = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--csv"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                for line in s.lines().skip(1) {
                    let cols: Vec<&str> = line.split(',').collect();
                    if cols.len() >= 2 {
                        if let Ok(bytes) = cols[1].trim().parse::<u64>() {
                            gpus.push(GpuInfo {
                                vendor: "AMD".into(),
                                name: cols[0].to_string(),
                                vram_bytes: bytes,
                            });
                        }
                    }
                }
            }
        }
    }
    gpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ram() {
        let info = detect();
        assert!(info.ram_total_bytes > 0);
        assert!(info.ram_available_bytes > 0);
        assert!(info.ram_available_bytes <= info.ram_total_bytes);
    }

    #[test]
    fn detects_cpu() {
        let info = detect();
        assert!(info.cpu_cores > 0);
        assert!(!info.cpu_model.is_empty());
    }
}
