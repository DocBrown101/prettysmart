use super::{StorageDevice, convert_lba_to_tb, nvme_controller_name};

#[test]
fn convert_lba_to_tb_formats_units_in_decimal_tb() {
    // NVMe units contain 1000 × 512 bytes; SATA LBAs contain 512 bytes.
    for (units, multiplier, expected) in [
        (1_953_125, 512000.0, "1.00 TB"),
        (1_953_125_000, 512.0, "1.00 TB"),
        (0, 512000.0, "0.00 TB"),
        (1_000_000, 512.0, "0.00 TB"),
        (7_812_500, 512000.0, "4.00 TB"),
    ] {
        assert_eq!(convert_lba_to_tb(units, multiplier), expected, "{units} units of {multiplier} bytes");
    }
}

#[test]
fn nvme_controller_name_strips_namespace() {
    assert_eq!(nvme_controller_name("nvme0n1"), "nvme0");
    assert_eq!(nvme_controller_name("nvme10n2"), "nvme10");
}

#[test]
fn nvme_controller_name_keeps_controller_and_unknown_names() {
    assert_eq!(nvme_controller_name("nvme0"), "nvme0");
    assert_eq!(nvme_controller_name("sda"), "sda");
}

#[test]
fn nvme_is_detected_from_native_and_usb_access_types() {
    for (interface, expected) in [("nvme", true), ("sntasmedia", true), ("sntjmicron", true), ("sntrealtek", true), ("sat", false)] {
        let device = StorageDevice {
            all_parts: format!("/dev/sda -d {interface}"),
            device_path: "/dev/sda".to_string(),
            short_device_name: "sda".to_string(),
            interface: interface.to_string(),
        };
        assert_eq!(device.is_nvme(), expected, "{interface}");
    }
}

#[cfg(unix)]
#[test]
fn read_error_bits_fail_but_health_warning_bits_preserve_measurements() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    for (process_status, json_status, failed) in [
        (0, 0, false),
        (1, 1, true),
        (2, 2, true),
        (4, 4, true),
        (0, 4, true),
        (4, 0, true),
        (8, 8, false),
        (16, 16, false),
        (32, 32, false),
        (64, 64, false),
        (128, 128, false),
    ] {
        let output = Output {
            status: ExitStatus::from_raw(process_status << 8),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let data = serde_json::json!({"smartctl":{"exit_status":json_status}});
        assert_eq!(super::smartctl_read_failed(&output, &data), failed, "{process_status}/{json_status}");
        assert!(super::smartctl_read_failed(&output, &serde_json::Value::Null));
    }
}
