use std::{fs, io, path::Path, process::Command};

#[derive(Debug, Clone)]
pub struct StorageDevice {
    pub all_parts: String,         // -> /dev/sda -d nvme
    pub device_path: String,       // -> /dev/sda
    pub short_device_name: String, // -> sda
    pub interface: String,         // -> nvme
}

pub fn find_storage_devices() -> Vec<StorageDevice> {
    let output = Command::new("smartctl").args(["--scan"]).output();

    if let Ok(output) = output
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);

        stdout
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }

                let mut parts = line.split('#'); // Kommentarteil (nach '#') entfernen
                let device_str = parts.next()?.trim();

                if device_str.starts_with("/dev/") {
                    let tokens: Vec<&str> = device_str.split_whitespace().collect();

                    if tokens.len() >= 3 && tokens[1] == "-d" {
                        let short_device_name = tokens[0].strip_prefix("/dev/")?.to_string();
                        let device_path = tokens[0].to_string();
                        let interface = tokens[2].to_lowercase();

                        Some(StorageDevice {
                            all_parts: device_str.to_string(),
                            device_path,
                            short_device_name,
                            interface,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    } else {
        return Vec::new();
    }
}

pub fn convert_data_units(units: i64) -> String {
    let bytes = units as f64 * 512000.0;
    let tb = bytes / 1e12;
    format!("{:.1} TB", tb)
}

/// Read from: /sys/class/nvme/nvme0/device/current_link_speed
pub fn get_nvme_pcie_info(nvme_name: &str) -> std::io::Result<(String, String)> {
    let base_path = format!("/sys/class/nvme/{}/device", nvme_name);
    let base = Path::new(&base_path);

    if !base.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("Base path '{}' existiert nicht", base_path)));
    }

    let speed_path = base.join("current_link_speed");
    let width_path = base.join("current_link_width");

    let speed = fs::read_to_string(&speed_path)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("Datei fehlt: {}", speed_path.display())))?
        .trim()
        .to_string();

    let width = fs::read_to_string(&width_path)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("Datei fehlt: {}", width_path.display())))?
        .trim()
        .to_string();

    Ok((speed, width))
}
