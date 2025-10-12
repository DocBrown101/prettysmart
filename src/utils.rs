use std::process::Command;

pub fn find_storage_devices() -> Vec<String> {
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
                    Some(device_str.to_string())
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
