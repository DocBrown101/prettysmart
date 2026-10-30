#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn smart_data_and_header_detect_nvme_from_access_type() {
    for (access_type, device_path, is_nvme) in [
        ("sntasmedia", "/dev/sda", true),
        ("sntjmicron", "/dev/sdb", true),
        ("sntrealtek", "/dev/sdc", true),
        ("nvme", "/dev/nvme0n1", true),
        ("sat", "/dev/sdd", false),
    ] {
        let directory = std::env::temp_dir().join(format!("prettysmart-protocol-{}-{access_type}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let data = json!({
            "model_name": "Test SSD",
            "serial_number": "TEST-SERIAL",
            "smart_status": { "passed": true },
            "nvme_version": { "string": "1.3" },
            "nvme_smart_health_information_log": {
                "available_spare": 100,
                "available_spare_threshold": 10,
                "percentage_used": 0,
                "data_units_written": 48067959,
                "power_on_hours": 2929,
                "media_errors": 0,
                "critical_comp_time": 2,
                "thermal_mgmt_temp1_total_time": 120
            },
            "ata_smart_attributes": { "table": [
                { "id": 9, "raw": { "value": 123 } }
            ] }
        });
        let smartctl = directory.join("smartctl");
        fs::write(
            &smartctl,
            format!(
                "#!/bin/sh\ncase \"$*\" in\n\
                 '--scan-open') printf '%s\\n' '{device_path} -d {access_type} # test device' ;;\n\
                 '-i -H -A -j {device_path} -d {access_type}') printf '%s\\n' '{data}' ;;\n\
                 *) exit 1 ;;\nesac\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&smartctl, fs::Permissions::from_mode(0o700)).unwrap();

        fs::create_dir(directory.join("prettysmart")).unwrap();
        fs::write(
            directory.join("prettysmart/snapshots.json"),
            json!({ "devices": { "TEST-SERIAL": [{
                "timestamp": 0,
                "metrics": { "nvme.thermal_mgmt_temp1_total_time": 60 }
            }] } })
            .to_string(),
        )
        .unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_prettysmart"))
            .env("PATH", &directory)
            .env("XDG_STATE_HOME", &directory)
            .env("LANG", "en_US.UTF-8")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        let snapshots = fs::read(directory.join("prettysmart/snapshots.json")).unwrap();
        fs::remove_dir_all(&directory).unwrap();

        assert!(output.status.success(), "{access_type}: {}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(&format!("{device_path} ({access_type})")), "{stdout}");
        let snapshots: Value = serde_json::from_slice(&snapshots).unwrap();
        let metrics = &snapshots["devices"]["TEST-SERIAL"][1]["metrics"];
        if is_nvme {
            assert!(stdout.contains("NVMe Version: 1.3"), "{stdout}");
            assert!(stdout.contains("24.61 TB"), "{stdout}");
            assert!(stdout.contains("2929 h"), "{stdout}");
            assert_eq!(metrics["nvme.power_on_hours"], 2929);
            assert_eq!(metrics["nvme.data_units_written"], 48067959_u64);
            assert_eq!(metrics["nvme.thermal_mgmt_temp1_total_time"], 120);
            let throttled = stdout
                .lines()
                .find(|line| line.contains("Overall throttled"))
                .unwrap();
            assert!(throttled.contains("120 s"), "{throttled}");
            assert!(throttled.contains("+60 s"), "{throttled}");
            let critical = stdout
                .lines()
                .find(|line| line.contains("Critical comp time"))
                .unwrap();
            assert!(critical.contains("2 min"), "{critical}");
            assert!(metrics["sata.power_on_hours"].is_null());
            if access_type != "nvme" {
                assert!(!stdout.contains("Transmission mode:"), "{stdout}");
            }
        } else {
            assert!(!stdout.contains("NVMe Version:"), "{stdout}");
            assert!(stdout.contains("123 h"), "{stdout}");
            assert_eq!(metrics["sata.power_on_hours"], 123);
            assert!(metrics["nvme.power_on_hours"].is_null());
        }
    }
}
