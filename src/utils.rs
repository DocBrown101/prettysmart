use glob::glob;

pub fn find_storage_devices() -> Vec<String> {
    let mut devices = vec![];
    for pattern in &["/dev/nvme*n1", "/dev/sd[a-z]"] {
        if let Ok(paths) = glob(pattern) {
            for path in paths.flatten() {
                devices.push(path.display().to_string());
            }
        }
    }
    devices
}

pub fn convert_data_units(units: i64) -> String {
    let bytes = units as f64 * 512000.0;
    let tb = bytes / 1e12;
    format!("{:.1} TB", tb)
}
