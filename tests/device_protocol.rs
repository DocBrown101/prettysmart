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
        fs::create_dir(&directory).expect("create test directory");
        let data = json!({
            "model_name": "Test SSD",
            "serial_number": "TEST-SERIAL",
            "smart_status": { "passed": true },
            "power_on_time": { "hours": 123 },
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
        .expect("write smartctl mock");
        fs::set_permissions(&smartctl, fs::Permissions::from_mode(0o700)).expect("make smartctl mock executable");

        fs::create_dir(directory.join("prettysmart")).expect("create snapshot directory");
        fs::write(
            directory.join("prettysmart/snapshots.json"),
            json!({ "devices": { "TEST-SERIAL": [{
                "timestamp": 0,
                "metrics": { "nvme.thermal_mgmt_temp1_total_time": 60 }
            }] } })
            .to_string(),
        )
        .expect("write previous snapshot");

        let output = Command::new(env!("CARGO_BIN_EXE_prettysmart"))
            .env("PATH", &directory)
            .env("XDG_STATE_HOME", &directory)
            .env("LANG", "en_US.UTF-8")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1")
            .output()
            .expect("run prettysmart with mocked smartctl");
        let snapshots = fs::read(directory.join("prettysmart/snapshots.json")).expect("read saved snapshots");
        fs::remove_dir_all(&directory).expect("remove test directory");

        assert!(output.status.success(), "{access_type}: {}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).expect("prettysmart output is valid UTF-8");
        assert!(stdout.contains(&format!("{device_path} ({access_type})")), "{stdout}");
        let snapshots: Value = serde_json::from_slice(&snapshots).expect("saved snapshots contain valid JSON");
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
                .expect("output contains thermal throttling duration");
            assert!(throttled.contains("120 s"), "{throttled}");
            assert!(throttled.contains("+60 s"), "{throttled}");
            let critical = stdout
                .lines()
                .find(|line| line.contains("Critical comp time"))
                .expect("output contains critical temperature duration");
            assert!(critical.contains("2 min"), "{critical}");
            assert!(metrics["sata.power_on_hours"].is_null());
            if access_type != "nvme" {
                assert!(!stdout.contains("Transmission mode:"), "{stdout}");
            }
        } else {
            assert!(!stdout.contains("NVMe Version:"), "{stdout}");
            assert!(stdout.contains("123 h"), "{stdout}");
            assert_eq!(metrics["sata.power_on_time.hours"], 123);
            assert!(metrics["nvme.power_on_hours"].is_null());
        }
    }
}
