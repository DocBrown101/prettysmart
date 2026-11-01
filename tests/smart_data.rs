#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Device {
    directory: PathBuf,
}

impl Device {
    fn new(data: Value) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!("prettysmart-data-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&directory).expect("create test directory");
        let device = Self { directory };
        device.write("scan", "/dev/sda -d sat # test\n");
        device.health(data, 0);
        device.write(
            "smartctl",
            r#"#!/bin/sh
root=${0%/*}
read_file() { while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$1"; }
case "$*" in
  --scan-open) read_file "$root/scan" ;;
  '-i -H -A -j /dev/sdb '*) read_file "$root/second.json" ;;
  '-i -H -A -j /dev/sda '*) read_file "$root/health.json"; exit "$(read_file "$root/health.status")" ;;
  *) exit 1 ;;
esac
"#,
        );
        fs::set_permissions(device.directory.join("smartctl"), fs::Permissions::from_mode(0o700)).expect("make mock executable");
        device
    }

    fn write(&self, file: &str, content: &str) {
        fs::write(self.directory.join(file), content).expect("write fixture");
    }

    fn health(&self, data: Value, status: u8) {
        self.write("health.json", &data.to_string());
        self.write("health.status", &status.to_string());
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_prettysmart"));
        command
            .env("PATH", &self.directory)
            .env("XDG_STATE_HOME", &self.directory)
            .env("LC_ALL", "C")
            .env_remove("LC_MESSAGES")
            .env("LANG", "en_US.UTF-8")
            .env("COLUMNS", "90")
            .env("NO_COLOR", "1");
        command
    }

    fn run(&self) -> Output {
        self.command()
            .output()
            .expect("run prettysmart")
    }

    fn snapshots(&self) -> Value {
        serde_json::from_slice(
            &fs::read(
                self.directory
                    .join("prettysmart/snapshots.json"),
            )
            .expect("read snapshots"),
        )
        .expect("parse snapshots")
    }

    fn seed(&self, metrics: Value) {
        fs::create_dir_all(self.directory.join("prettysmart")).expect("create snapshot directory");
        self.write(
            "prettysmart/snapshots.json",
            &json!({"devices":{"TEST":[{"timestamp":123,"metrics":metrics}]}}).to_string(),
        );
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("clean test directory");
    }
}

fn ata(attrs: Value) -> Value {
    json!({
        "device":{"protocol":"ATA"}, "serial_number":"TEST", "model_name":"Test SSD",
        "firmware_version":"FW1", "user_capacity":{"bytes":1000000000000_u64},
        "smart_status":{"passed":true}, "power_on_time":{"hours":200},
        "ata_smart_attributes":{"table":attrs}
    })
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 output")
}

fn row<'a>(output: &'a str, label: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label}: {output}"))
}

fn current_value<'a>(output: &'a str, label: &str) -> &'a str {
    row(output, label)
        .split('│')
        .nth(2)
        .expect("value column")
        .trim()
}

fn status<'a>(output: &'a str, label: &str) -> &'a str {
    row(output, label)
        .split('│')
        .nth(4)
        .expect("status column")
        .trim()
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn column_text(output: &str, column: usize) -> String {
    output
        .lines()
        .filter_map(|line| line.split('│').nth(column))
        .map(str::trim)
        .collect()
}

#[test]
fn read_failures_keep_history_and_continue_with_other_devices() {
    let good = ata(json!([{"id":12,"name":"Power_Cycle_Count","raw":{"value":7}}]));
    let device = Device::new(good.clone());
    device.seed(json!({"sata.power_cycles":5}));
    let original = device.snapshots()["devices"]["TEST"].clone();
    device.write("scan", "/dev/sda -d sat\n/dev/sdb -d sat\n");
    let mut second = good.clone();
    second["serial_number"] = json!("SECOND");
    device.write("second.json", &second.to_string());
    for (process_status, json_status) in [(1, 1), (2, 2), (4, 4), (0, 4), (4, 0)] {
        let mut failed = good.clone();
        failed["smartctl"] = json!({"exit_status":json_status,"messages":[{"string":"device read failed"}]});
        device.health(failed, process_status);
        let output = device.run();
        assert!(!output.status.success());
        assert!(stdout(&output).contains("/dev/sdb"));
        assert!(stdout(&output).contains("Unavailable"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("/dev/sda: device read failed"));
        assert_eq!(device.snapshots()["devices"]["TEST"], original);
    }
    assert_eq!(
        device.snapshots()["devices"]["SECOND"]
            .as_array()
            .expect("second snapshots")
            .len(),
        5
    );
}

#[test]
fn health_failure_bits_preserve_measurements_and_show_failed_attributes() {
    let mut data = ata(json!([
        {"id":1,"name":"Raw_Read_Error_Rate","value":5,"thresh":10,"when_failed":"now","raw":{"value":0}},
        {"id":184,"name":"End-to-End_Error","value":99,"thresh":90,"when_failed":"past","raw":{"value":1}},
        {"id":187,"name":"Reported_Uncorrect","raw":{"value":2}}
    ]));
    data["smart_status"]["passed"] = json!(false);
    let device = Device::new(data.clone());
    for status in [8, 16, 32, 64, 128] {
        device.health(data.clone(), status);
        let output = device.run();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = stdout(&output);
        assert!(row(&text, "SMART overall status").contains("CRITICAL"));
        assert!(row(&text, "Raw_Read_Error_Rate:").contains("CRITICAL"));
        assert!(compact(&column_text(&text, 1)).contains("Thresholdfailednow"));
        assert!(row(&text, "End-to-End_Error").contains("WARNING"));
        assert!(!text.contains("Threshold failed in the past"));
        assert!(row(&text, "Reported uncorrectable errors").contains("CRITICAL"));
    }
    assert_eq!(
        device.snapshots()["devices"]["TEST"]
            .as_array()
            .expect("snapshots")
            .len(),
        5
    );
}

#[test]
fn ata_properties_and_values_are_compact_and_zero_measurements_are_hidden() {
    let mut data = ata(json!([
        {"id":1,"name":"Raw_Read_Error_Rate","value":100,"thresh":10,"when_failed":"past","raw":{"value":0}},
        {"id":10,"name":"Spin_Retry_Count","value":100,"thresh":10,"when_failed":"past","raw":{"value":0}},
        {"id":184,"name":"End-to-End_Error","value":99,"thresh":90,"when_failed":"past","raw":{"value":1}},
        {"id":200,"name":"Unknown_Attribute","when_failed":"past"},
        {"id":201,"name":"Vendor_Attribute_123","raw":{"value":7}},
        {"id":202,"name":"Program_Fail_Cnt_Total","raw":{"string":"0"}},
        {"id":233,"name":"Media_Wearout_Indicator","value":99,"when_failed":"past","raw":{"value":0}},
        {"id":241,"name":"Host_Writes_32MiB","when_failed":"past","raw":{"value":0}}
    ]));
    data["model_name"] = json!("INTEL SSDSC2KB480G8");
    let device = Device::new(data);
    for (locale, warning, endurance) in [
        ("C", "WARNING", "Estimated remaining write endurance"),
        ("de_DE.UTF-8", "WARNUNG", "Geschätzte Rest-Schreibausdauer"),
    ] {
        let output = device
            .command()
            .env("LC_ALL", locale)
            .output()
            .expect("run localized CLI");
        assert!(output.status.success());
        let text = stdout(&output);
        for label in ["Raw_Read_Error_Rate", "Spin Retry Count", "Program_Fail_Cnt_Total", "Host_Writes_MiB"] {
            assert!(!text.contains(label), "{text}");
        }
        assert_eq!(current_value(&text, "Vendor_Attribute_"), "7");
        assert_eq!(
            row(&text, "Vendor_Attribute_")
                .split('│')
                .nth(4)
                .expect("status column")
                .trim(),
            "—"
        );
        for label in ["Vendor_Attribute_", endurance] {
            assert!(!row(&text, label).contains(warning), "{text}");
        }
        assert!(row(&text, "End-to-End_Error").contains(warning), "{text}");
        assert!(row(&text, "Unknown_Attribute").contains(warning), "{text}");
        assert!(!text.contains("Threshold failed in the past"));
        assert!(!text.contains("Grenzwert früher verletzt"));
        for line in text
            .lines()
            .filter(|line| line.matches('│').count() == 5)
        {
            let property = line
                .split('│')
                .nth(1)
                .expect("property column");
            assert!(
                !property
                    .chars()
                    .any(|ch| ch.is_ascii_digit()),
                "{property}"
            );
        }
        assert_eq!(
            device.snapshots()["devices"]["TEST"][0]["metrics"]["sata.241.Host_Writes_32MiB.units_33554432"],
            0
        );
    }
}

#[test]
fn malformed_missing_and_unsupported_data_are_not_successful() {
    for data in [json!({}), json!(null), json!({"device":{"protocol":"SCSI"},"smart_status":{"passed":true}})] {
        let device = Device::new(data);
        let output = device.run();
        assert!(!output.status.success());
        assert!(stdout(&output).contains("Unavailable"));
        assert!(
            device.snapshots()["devices"]
                .as_object()
                .expect("devices")
                .is_empty()
        );
    }
    let device = Device::new(json!({}));
    device.write("health.json", "not JSON");
    assert!(!device.run().status.success());
}

#[test]
fn sata_names_units_and_decreasing_counts_are_respected() {
    let device = Device::new(ata(json!([
        {"id":9,"name":"Power_On_Seconds","raw":{"value":720000}},
        {"id":197,"name":"Current_Pending_Sector","raw":{"value":3}},
        {"id":198,"name":"Offline_Uncorrectable","raw":{"value":1}},
        {"id":177,"name":"Read_Retry_Cnt","value":200,"raw":{"value":8}},
        {"id":233,"name":"NAND_GiB_Written","value":100,"raw":{"value":1024}},
        {"id":241,"name":"Host_Writes_32MiB","raw":{"value":0}},
        {"id":242,"name":"Lifetime_Reads_GiB","raw":{"value":1024}},
        {"id":246,"name":"Total_LBAs_Written","raw":{"value":1000000000}}
    ])));
    device.seed(json!({"sata.power_on_hours":720000,"sata.pending_sectors":8,"sata.offline_uncorrectable_sectors":2}));
    let output = device.run();
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(row(&text, "Operating hours").contains("200 h"));
    assert!(!row(&text, "Operating hours").contains("reset?"));
    assert!(row(&text, "Pending sectors").contains("-5"));
    assert!(row(&text, "Offline uncorrectable sectors").contains("-1"));
    assert!(!row(&text, "Read_Retry_Cnt").contains('%'));
    assert!(row(&text, "NAND_GiB_Written").contains("1.10 TB"));
    assert!(row(&text, "Lifetime_Reads_GiB").contains("1.10 TB"));
    assert_eq!(current_value(&text, "Total_LBAs_Written"), "1000000000");
    assert!(text.contains("1000.00 GB"));
    assert!(text.contains("Firmware: FW1"));
    assert!(text.contains("Serial number: TEST"));
    let snapshots = device.snapshots();
    let metrics = &snapshots["devices"]["TEST"][1]["metrics"];
    assert_eq!(metrics["sata.power_on_time.hours"], 200);
    assert_eq!(metrics["sata.241.Host_Writes_32MiB.units_33554432"], 0);
    assert!(metrics["sata.drive_health_remaining"].is_null());
}

#[test]
fn nand_writes_use_decimal_gb_and_stay_neutral() {
    for model in ["WDC  WDS500G2B0A-00SM50", "Another SSD"] {
        let mut data = ata(json!([
            {"id":233,"name":"NAND_GB_Written_TLC","value":100,"thresh":0,"raw":{"value":0}},
            {"id":234,"name":"NAND_GB_Written_SLC","value":100,"thresh":0,"raw":{"value":0}}
        ]));
        data["model_name"] = json!(model);
        let device = Device::new(data.clone());
        let zero = device.run();
        assert!(zero.status.success());
        assert!(!stdout(&zero).contains("NAND"));
        data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(6805);
        data["ata_smart_attributes"]["table"][1]["raw"]["value"] = json!(15295);
        device.health(data.clone(), 0);
        for (locale, title) in [("C", "NAND written"), ("de_DE.UTF-8", "NAND geschrieben")] {
            let output = device
                .command()
                .env("LC_ALL", locale)
                .output()
                .expect("run localized CLI");
            assert!(output.status.success());
            let text = stdout(&output);
            for (kind, value) in [("TLC", "6.80 TB"), ("SLC", "15.29 TB")] {
                let label = format!("{title} ({kind})");
                assert_eq!(current_value(&text, &label), value);
                assert_eq!(
                    row(&text, &label)
                        .split('│')
                        .nth(4)
                        .expect("status column")
                        .trim(),
                    "—"
                );
            }
        }
        let snapshots = device.snapshots();
        assert_eq!(snapshots["devices"]["TEST"][0]["metrics"]["sata.233.NAND_GB_Written_TLC.units_1000000000"], 0);
        assert_eq!(
            snapshots["devices"]["TEST"][1]["metrics"]["sata.233.NAND_GB_Written_TLC.units_1000000000"],
            6805
        );
        assert_eq!(
            snapshots["devices"]["TEST"][1]["metrics"]["sata.234.NAND_GB_Written_SLC.units_1000000000"],
            15295
        );
        data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(6806);
        data["ata_smart_attributes"]["table"][1]["raw"]["value"] = json!(15297);
        device.health(data, 0);
        let changed = device.run();
        assert!(changed.status.success());
        let text = stdout(&changed);
        assert!(row(&text, "NAND written (TLC)").contains("+1.00 GB"), "{text}");
        assert!(row(&text, "NAND written (SLC)").contains("+2.00 GB"), "{text}");
        assert!(!text.contains("WARNING") && !text.contains("CRITICAL"), "{text}");
    }
}

#[test]
fn nand_gb_writes_are_recognized_by_name_regardless_of_model_or_id() {
    for (model, tlc_id, slc_id) in [
        (Some("Test SSD"), 233, 234),
        (Some("WDC WDS500G3B0A-00SM50"), 233, 234),
        (Some("WDC WDS500G2B0A-00SM50"), 234, 233),
        (None, 250, 251),
    ] {
        let mut data = ata(json!([
            {"id":tlc_id,"name":"NAND_GB_Written_TLC","raw":{"value":6805}},
            {"id":slc_id,"name":"NAND_GB_Written_SLC","raw":{"value":15295}}
        ]));
        data["model_name"] = json!(model);
        let device = Device::new(data);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        assert_eq!(current_value(&text, "NAND written (TLC)"), "6.80 TB");
        assert_eq!(current_value(&text, "NAND written (SLC)"), "15.29 TB");
        assert!(!text.contains("NAND_GB_Written"), "{text}");
    }
}

#[test]
fn nand_threshold_failures_remain_visible_and_critical_even_at_zero() {
    let mut data = ata(json!([{"id":233,"name":"NAND_GB_Written_TLC","value":5,"thresh":10,"raw":{"value":0}}]));
    data["model_name"] = json!("WDC  WDS500G2B0A-00SM50");
    let device = Device::new(data);
    let output = device.run();
    assert!(output.status.success());
    assert!(row(&stdout(&output), "NAND written (TLC)").contains("CRITICAL"));
}

#[test]
fn recognized_remaining_endurance_uses_profile_specific_values() {
    for (model, family, attr, expected, expected_status) in [
        (
            "CT1000MX500SSD1",
            "Crucial/Micron Client SSDs",
            json!({"id":202,"name":"Percent_Lifetime_Remain","value":85,"raw":{"value":15}}),
            85,
            "—",
        ),
        (
            "KINGSTON SUV400S37240G",
            "Kingston SSDs",
            json!({"id":169,"name":"Remaining_Lifetime_Perc","value":25,"raw":{"value":25}}),
            25,
            "WARNING",
        ),
        (
            "INTEL SSDSC2KB480G8",
            "Intel S4x00",
            json!({"id":245,"name":"Percent_Life_Remaining","value":64,"raw":{"value":10}}),
            10,
            "CRITICAL",
        ),
        (
            "SSDSC2KB480G7R",
            "Dell Certified Intel S4x00/D3-S4x10 Series SSDs",
            json!({"id":245,"name":"Percent_Life_Remaining","value":82,"raw":{"value":5}}),
            5,
            "CRITICAL",
        ),
    ] {
        let mut data = ata(json!([attr]));
        data["model_name"] = json!(model);
        data["model_family"] = json!(family);
        let device = Device::new(data);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        let endurance = row(&text, "Estimated remaining write endurance");
        assert!(endurance.contains(&format!("{expected} %")), "{endurance}");
        assert!(status(&text, "Estimated remaining write endurance").ends_with(expected_status), "{endurance}");
        let snapshots = device.snapshots();
        let key = format!(
            "sata.{}.{}.remaining_percent",
            attr["id"],
            attr["name"]
                .as_str()
                .expect("attribute name")
        );
        assert_eq!(snapshots["devices"]["TEST"][0]["metrics"][key], expected);
        let unchanged = device.run();
        assert!(unchanged.status.success());
        let unchanged = stdout(&unchanged);
        assert_eq!(
            status(&unchanged, "Estimated remaining write endurance"),
            status(&text, "Estimated remaining write endurance")
        );
    }
    let device = Device::new(ata(json!([
        {"id":170,"name":"Grown_Failing_Block_Ct","value":100,"raw":{"value":3}},
        {"id":231,"name":"SSD_Life_Left","value":4,"raw":{"value":96}},
        {"id":233,"name":"Media_Wearout_Indicator","value":100,"raw":{"value":7}}
    ])));
    let output = device.run();
    assert!(output.status.success());
    let text = stdout(&output);
    for (name, raw) in [("Grown_Failing_Block_Ct", "3"), ("SSD_Life_Left", "96"), ("Media_Wearout_Indicator", "7")] {
        assert!(!row(&text, name).contains('%'));
        assert_eq!(current_value(&text, name), raw);
    }
}

#[test]
fn intel_remaining_endurance_validates_raw_percentage_without_normalized_fallback() {
    for (raw, expected) in [(json!(0), Some(0)), (json!(100), Some(100)), (json!(101), None), (json!(null), None)] {
        let mut data = ata(json!([{"id":245,"name":"Percent_Life_Remaining","value":64,"raw":{"value":raw}}]));
        data["model_name"] = json!("INTEL SSDSC2KB480G8");
        let device = Device::new(data);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        if let Some(percent) = expected {
            assert!(row(&text, "Estimated remaining write endurance").contains(&format!("{percent} %")), "{text}");
        } else {
            assert!(!text.contains("Estimated remaining write endurance"), "{text}");
            assert_eq!(
                current_value(&text, "Percent_Life_Remaining"),
                if raw.is_null() { "Unavailable" } else { "101" }
            );
        }
    }
}

#[test]
fn intel_error_counter_aliases_warn_and_keep_tracking_changes() {
    let device = Device::new(ata(json!([
        {"id":197,"name":"Pending_Sector_Count","value":100,"thresh":0,"raw":{"value":2}},
        {"id":199,"name":"CRC_Error_Count","value":100,"thresh":0,"raw":{"value":3}}
    ])));
    device.seed(json!({"sata.pending_sectors":1,"sata.udma_crc_errors":1}));
    let output = device.run();
    assert!(output.status.success());
    let text = stdout(&output);
    let pending = row(&text, "Pending sectors");
    assert!(pending.contains("CRITICAL") && pending.contains("+1"), "{pending}");
    let crc = row(&text, "UDMA CRC-err");
    assert!(crc.contains("WARNING") && crc.contains("+2"), "{crc}");
    let snapshots = device.snapshots();
    assert_eq!(snapshots["devices"]["TEST"][1]["metrics"]["sata.pending_sectors"], 2);
    assert_eq!(snapshots["devices"]["TEST"][1]["metrics"]["sata.udma_crc_errors"], 3);
}

#[test]
fn ata_counter_warnings_require_a_changed_positive_count() {
    for (id, name, key, label) in [
        (199, "UDMA_CRC_Error_Count", "sata.udma_crc_errors", "UDMA CRC-err"),
        (199, "CRC_Error_Count", "sata.udma_crc_errors", "UDMA CRC-err"),
        (10, "Spin_Retry_Count", "sata.spin_retry_count", "Spin Retry Count"),
    ] {
        for (previous, current, warning) in [
            (None, 0, false),
            (None, 3, false),
            (Some(3), 3, false),
            (Some(0), 3, true),
            (Some(3), 5, true),
            (Some(5), 3, true),
            (Some(3), 0, false),
        ] {
            let device = Device::new(ata(json!([
                {"id":id,"name":name,"value":100,"thresh":0,"when_failed":"past","raw":{"value":current}}
            ])));
            if let Some(previous) = previous {
                device.seed(json!({key:previous}));
            }
            let output = device.run();
            assert!(output.status.success());
            let text = stdout(&output);
            assert_eq!(text.contains("WARNING"), warning, "{name}: {previous:?} -> {current}: {text}");
            if current == 0 {
                assert!(!text.contains(label), "{text}");
            } else {
                assert_eq!(current_value(&text, label), current.to_string());
            }
            let unchanged = device.run();
            assert!(unchanged.status.success());
            assert!(!stdout(&unchanged).contains("WARNING"));
        }
    }
}

#[test]
fn hidden_ata_zero_values_stay_in_history_and_positive_values_stay_visible() {
    let mut data = ata(json!([
        {"id":5,"name":"Reallocated_Sector_Ct","raw":{"value":0}},
        {"id":12,"name":"Power_Cycle_Count","raw":{"value":0}},
        {"id":187,"name":"Reported_Uncorrect","raw":{"value":0}},
        {"id":197,"name":"Current_Pending_Sector","raw":{"value":0}},
        {"id":198,"name":"Offline_Uncorrectable","raw":{"value":0}},
        {"id":199,"name":"UDMA_CRC_Error_Count","raw":{"value":0}},
        {"id":200,"name":"Used_Rsvd_Blk_Cnt_Tot","raw":{"value":0}},
        {"id":241,"name":"Host_Writes_GiB","raw":{"value":0}}
    ]));
    data["power_on_time"]["hours"] = json!(0);
    let device = Device::new(data.clone());
    for current in [0, 2, 2, 0, 1] {
        for attr in data["ata_smart_attributes"]["table"]
            .as_array_mut()
            .expect("attributes")
        {
            attr["raw"]["value"] = json!(current);
        }
        device.health(data.clone(), 0);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        assert!(!text.contains("Operating hours"));
        for label in [
            "Reallocated Sectors",
            "Power cycles",
            "Reported uncorrectable errors",
            "Pending sectors",
            "Offline uncorrectable sectors",
            "UDMA CRC-err",
            "Used_Rsvd_Blk_Cnt_Tot",
            "Host_Writes_GiB",
        ] {
            assert_eq!(text.contains(label), current > 0, "{label}: {text}");
        }
        let snapshots = device.snapshots();
        let metrics = &snapshots["devices"]["TEST"]
            .as_array()
            .expect("snapshots")
            .last()
            .expect("latest snapshot")["metrics"];
        for key in [
            "sata.reallocated_sectors",
            "sata.power_cycles",
            "sata.reported_uncorrectable",
            "sata.pending_sectors",
            "sata.offline_uncorrectable_sectors",
            "sata.udma_crc_errors",
            "sata.241.Host_Writes_GiB.units_1073741824",
        ] {
            assert_eq!(metrics[key], current);
        }
        assert_eq!(metrics["sata.power_on_time.hours"], 0);
        if current == 1 {
            assert!(row(&text, "UDMA CRC-err").contains("+1"), "{text}");
        }
    }
}

#[test]
fn unchanged_udma_crc_threshold_failures_remain_critical() {
    let device = Device::new(ata(json!([
        {"id":199,"name":"UDMA_CRC_Error_Count","value":5,"thresh":10,"raw":{"value":3}}
    ])));
    device.seed(json!({"sata.udma_crc_errors":3}));
    let output = device.run();
    assert!(output.status.success());
    assert!(row(&stdout(&output), "UDMA CRC-err").contains("CRITICAL"));
}

#[test]
fn nvme_zero_values_are_hidden_except_for_threshold_failures_and_counters_warn_on_change() {
    let mut data = json!({
        "device":{"protocol":"NVMe"},"serial_number":"TEST","smart_status":{"passed":true},
        "nvme_smart_health_information_log":{
            "available_spare":0,"available_spare_threshold":10,"percentage_used":100,
            "power_on_hours":0,"power_cycles":0,"data_units_written":0,"media_errors":0,
            "num_err_log_entries":0,"critical_comp_time":0,"thermal_mgmt_temp1_total_time":0
        }
    });
    let device = Device::new(data.clone());
    for (current, warning) in [(0, false), (3, true), (3, false)] {
        data["nvme_smart_health_information_log"]["num_err_log_entries"] = json!(current);
        device.health(data.clone(), 0);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        assert!(row(&text, "Spare blocks").contains("CRITICAL"));
        assert!(row(&text, "Estimated remaining write endurance").contains("CRITICAL"));
        for label in [
            "Operating hours",
            "Power cycles",
            "Data written",
            "Media Errors",
            "Critical comp time",
            "Overall throttled",
        ] {
            assert!(!text.contains(label), "{text}");
        }
        assert_eq!(text.contains("Num err log entries"), current > 0, "{text}");
        assert_eq!(text.contains("WARNING"), warning, "{text}");
    }
}

#[test]
fn nvme_wear_above_100_and_all_warning_bits_are_visible() {
    let device = Device::new(json!({
        "device":{"protocol":"NVMe"},"serial_number":"TEST","smart_status":{"passed":false},
        "nvme_smart_health_information_log":{"percentage_used":150,"critical_warning":96,"thermal_mgmt_temp1_total_time":0}
    }));
    let output = device.run();
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(row(&text, "Estimated endurance used").contains("150 %"));
    assert!(row(&text, "Estimated remaining write endurance").contains("0 % remaining"));
    assert!(compact(&column_text(&text, 1)).contains("Persistentmemoryunreliable/read-only"));
    assert_eq!(current_value(&text, "Unknown warning bits"), "0x40");
    assert!(status(&text, "Unknown warning bits").contains("CRITICAL"));
    assert_eq!(device.snapshots()["devices"]["TEST"][0]["metrics"]["nvme.thermal_mgmt_temp1_total_time"], 0);
}

#[test]
fn capacity_shares_the_model_line_and_serial_shares_the_firmware_line() {
    for (protocol, model, capacity) in [
        ("ATA", "Test SSD", Some(1000000000000_u64)),
        ("NVMe", "Test SSD", Some(1000000000000_u64)),
        (
            "ATA",
            "A very long device model name that must not collide with the capacity at the right edge",
            Some(1000000000000_u64),
        ),
        ("ATA", "Test SSD", None),
    ] {
        let mut data = ata(json!([{"id":200,"name":"A_very_long_vendor_attribute_name_to_make_the_table_wider_than_the_default_width","raw":{"value":1}}]));
        data["device"]["protocol"] = json!(protocol);
        data["model_name"] = json!(model);
        data["user_capacity"]["bytes"] = json!(capacity);
        if protocol == "NVMe" {
            data["nvme_smart_health_information_log"] = json!({"power_on_hours":200});
        }
        let device = Device::new(data);
        for (locale, capacity_label, model_label) in [
            ("C", "Capacity", if protocol == "NVMe" { "Model Number" } else { "Device Model" }),
            ("de_DE.UTF-8", "Kapazität", if protocol == "NVMe" { "Modellnummer" } else { "Gerätemodell" }),
        ] {
            let output = device
                .command()
                .env("LC_ALL", locale)
                .output()
                .expect("run localized CLI");
            assert!(output.status.success());
            let text = stdout(&output);
            let model_row = row(&text, &format!("{model_label}:"));
            if capacity.is_some() {
                assert!(model_row.ends_with(&format!("{capacity_label}: 1000.00 GB │")), "{text}");
                assert_eq!(
                    text.matches(&format!("{capacity_label}:"))
                        .count(),
                    1
                );
            } else {
                assert!(!text.contains(&format!("{capacity_label}:")), "{text}");
            }
            let header: Vec<_> = text
                .lines()
                .skip_while(|line| !line.contains("/dev/sda"))
                .take_while(|line| line.starts_with('│'))
                .collect();
            assert_eq!(header.len(), if model.len() > 50 { 4 } else { 3 }, "{text}");
            let header_text = header
                .join("\n")
                .replace(&format!("{capacity_label}: 1000.00 GB"), "")
                .replace('│', "");
            assert!(compact(&header_text).contains(&compact(model)), "{text}");
            assert!(
                header[2..]
                    .iter()
                    .any(|line| line.contains("TEST") && line.contains("FW1")),
                "{text}"
            );
        }
    }
}

#[test]
fn protocol_version_is_inline_with_the_model_and_uses_the_interface_color() {
    for (protocol, version_field, version, english_label, german_label) in [
        ("ATA", "sata_version", " SATA 3.3 ", "Device Model", "Gerätemodell"),
        ("NVMe", "nvme_version", " 1.3 ", "Model Number", "Modellnummer"),
    ] {
        for (model, version) in [(Some("Test SSD"), Some(version)), (Some("Test SSD"), None), (None, Some(version))] {
            let mut data = ata(json!([{"id":12,"name":"Power_Cycle_Count","raw":{"value":1}}]));
            data["device"]["protocol"] = json!(protocol);
            data["model_name"] = json!(model);
            data[version_field]["string"] = json!(version);
            if protocol == "NVMe" {
                data["nvme_smart_health_information_log"] = json!({"power_on_hours":200});
            }
            let device = Device::new(data);
            for (locale, label, unavailable) in [("C", english_label, "Unavailable"), ("de_DE.UTF-8", german_label, "Nicht verfügbar")] {
                for color in [false, true] {
                    let output = device
                        .command()
                        .env("LC_ALL", locale)
                        .env("CLICOLOR_FORCE", if color { "1" } else { "0" })
                        .output()
                        .expect("run localized CLI");
                    assert!(output.status.success());
                    let text = stdout(&output);
                    let model_label = format!("{label}: {}", model.unwrap_or(unavailable));
                    let model_row = row(&text, &model_label);
                    if let Some(version) = version {
                        let suffix = match (protocol, color) {
                            ("NVMe", true) => format!(" (\x1b[36mNVMe {}\x1b[0m)", version.trim()),
                            ("NVMe", false) => format!(" (NVMe {})", version.trim()),
                            (_, true) => format!(" (\x1b[36m{}\x1b[0m)", version.trim()),
                            (_, false) => format!(" ({})", version.trim()),
                        };
                        assert!(model_row.contains(&format!("{model_label}{suffix}")), "{text}");
                        assert!(model_row.ends_with("1000.00 GB │"), "{text}");
                    } else {
                        assert!(!model_row.contains('('), "{text}");
                    }
                    if color {
                        assert!(text.contains("(\x1b[36msat\x1b[0m)"), "{text}");
                    }
                    assert!(!text.contains("SATA-Version:"), "{text}");
                    assert!(!text.contains("SATA Version:"), "{text}");
                    assert!(!text.contains("NVMe-Version:"), "{text}");
                    assert!(!text.contains("NVMe Version:"), "{text}");
                }
            }
        }
    }
}

#[test]
fn locale_precedence_uses_first_nonempty_variable() {
    let device = Device::new(ata(json!([{"id":12,"name":"Power_Cycle_Count","raw":{"value":1}}])));
    for (all, messages, lang, expected) in [
        ("C", "de_DE.UTF-8", "de_DE.UTF-8", "Storage Media Diagnostics"),
        ("", "de_DE.UTF-8", "en_US.UTF-8", "Speichermedien-Diagnose"),
        ("", "", "de_DE.UTF-8", "Speichermedien-Diagnose"),
    ] {
        let output = device
            .command()
            .env("LC_ALL", all)
            .env("LC_MESSAGES", messages)
            .env("LANG", lang)
            .output()
            .expect("run localized CLI");
        assert!(output.status.success());
        assert!(stdout(&output).contains(expected));
    }
}

#[test]
fn verified_lba_units_and_zero_baselines_produce_correct_deltas() {
    for (model, family, id) in [
        ("Samsung SSD 870 EVO 1TB", "Samsung based SSDs", 241),
        ("CT1000MX500SSD1", "Crucial/Micron Client SSDs", 246),
    ] {
        let mut data = ata(json!([{"id":id,"name":"Total_LBAs_Written","raw":{"value":0}}]));
        data["model_name"] = json!(model);
        data["model_family"] = json!(family);
        data["logical_block_size"] = json!(4096);
        let device = Device::new(data.clone());
        assert!(device.run().status.success());
        data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(1953125000_u64);
        device.health(data, 0);
        let output = device.run();
        assert!(output.status.success());
        let text = stdout(&output);
        let value = row(&text, "Total_LBAs_Written");
        assert!(value.contains("1.00 TB"), "{value}");
        assert!(value.contains("+1.00 TB"), "{value}");
    }
}

#[test]
fn undecoded_hours_are_not_guessed() {
    let mut data = ata(json!([{"id":9,"name":"Power_On_Hours_and_Msec","raw":{"value":4294967301_u64,"string":"5h+00m+01s"}}]));
    data.as_object_mut()
        .expect("object")
        .remove("power_on_time");
    let device = Device::new(data);
    let output = device.run();
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(!text.contains("Operating hours"));
    assert!(row(&text, "Power_On_Hours_and_Msec").contains("5h+00m+01s"));
}

#[test]
fn vendor_section_follows_known_neutral_metrics_and_never_contains_alerts() {
    let mut data = ata(json!([
        {"id":200,"name":"Vendor_Field","raw":{"value":9,"string":"9 (vendor encoding)"}},
        {"id":233,"name":"NAND_GB_Written_TLC","raw":{"value":6805}},
        {"id":184,"name":"End-to-End_Error","when_failed":"past","raw":{"value":1}},
        {"id":199,"name":"UDMA_CRC_Error_Count","raw":{"value":1}},
        {"id":201,"name":"Another_Vendor_Field","raw":{"value":0}},
        {"id":202,"name":"Missing_Vendor_Value"}
    ]));
    data["power_on_time"]["hours"] = json!(0);
    let device = Device::new(data);
    for (locale, section, nand, warning, unavailable) in [
        (
            "C",
            "Other vendor values · without own assessment",
            "NAND written (TLC)",
            "WARNING",
            "Unavailable",
        ),
        (
            "de_DE.UTF-8",
            "Weitere Herstellerwerte · ohne eigene Bewertung",
            "NAND geschrieben (TLC)",
            "WARNUNG",
            "Nicht verfügbar",
        ),
    ] {
        device.seed(json!({"sata.udma_crc_errors":0}));
        let output = device
            .command()
            .env("LC_ALL", locale)
            .output()
            .expect("run localized CLI");
        assert!(output.status.success());
        let text = stdout(&output);
        let (known, raw) = text
            .split_once(section)
            .expect("vendor section");
        assert!(known.contains(nand), "{text}");
        assert_eq!(status(&text, nand), "—");
        assert!(status(known, "End-to-End_Error").contains(warning));
        assert!(status(known, "UDMA").contains(warning));
        assert!(!raw.contains(warning), "{text}");
        assert_eq!(status(raw, "Vendor_Field"), "—");
        assert!(compact(&column_text(raw, 2)).contains("9(vendorencoding)"), "{text}");
        assert!(!raw.contains("Another_Vendor_Field"), "{text}");
        assert_eq!(current_value(raw, "Missing_Vendor_Value"), unavailable);
    }
}

#[test]
fn zero_baselines_show_changes_without_turning_normal_metrics_into_warnings() {
    for protocol in ["ATA", "NVMe"] {
        for (locale, warning, critical, cycles, written, error) in [
            ("C", "WARNING", "CRITICAL", "Power cycles", "Data written", "Num err log entries"),
            (
                "de_DE.UTF-8",
                "WARNUNG",
                "KRITISCH",
                "Einschaltzyklen",
                "Daten geschrieben",
                "Fehlerprotokoll-Einträge",
            ),
        ] {
            let mut data = ata(json!([
                {"id":12,"name":"Power_Cycle_Count","raw":{"value":0}},
                {"id":199,"name":"UDMA_CRC_Error_Count","raw":{"value":0}},
                {"id":5,"name":"Reallocated_Sector_Ct","raw":{"value":0}},
                {"id":241,"name":"Host_Writes_GiB","raw":{"value":0}}
            ]));
            data["device"]["protocol"] = json!(protocol);
            data["nvme_smart_health_information_log"] =
                json!({"power_cycles":0,"data_units_written":0,"num_err_log_entries":0,"media_errors":0,"unsafe_shutdowns":0});
            let device = Device::new(data.clone());
            assert!(device.run().status.success());
            for attr in data["ata_smart_attributes"]["table"]
                .as_array_mut()
                .expect("attributes")
            {
                attr["raw"]["value"] = json!(1);
            }
            for value in data["nvme_smart_health_information_log"]
                .as_object_mut()
                .expect("NVMe log")
                .values_mut()
            {
                *value = json!(1);
            }
            device.health(data, 0);
            let run = || {
                stdout(
                    &device
                        .command()
                        .env("LC_ALL", locale)
                        .output()
                        .expect("run CLI"),
                )
            };
            let text = run();
            let error = if protocol == "ATA" { "UDMA" } else { error };
            let written = if protocol == "ATA" { "Host_Writes_GiB" } else { written };
            assert_eq!(status(&text, cycles), "—");
            assert_eq!(status(&text, written), "—");
            assert!(row(&text, cycles).contains("+1"), "{text}");
            assert!(
                row(&text, written).contains(if protocol == "ATA" { "+1.07 GB" } else { "+512.00 KB" }),
                "{text}"
            );
            assert!(status(&text, error).contains(warning), "{text}");
            assert!(row(&text, error).contains("+1"), "{text}");
            let critical_counter = if protocol == "ATA" {
                "Reallocated Sectors"
            } else if locale == "C" {
                "Media Errors"
            } else {
                "Medienfehler"
            };
            assert!(status(&text, critical_counter).contains(critical), "{text}");
            if protocol == "NVMe" {
                assert_eq!(status(&text, if locale == "C" { "Unsafe Shutdowns" } else { "Unsichere Abschaltungen" }), "—");
            }
            let unchanged = run();
            for label in [cycles, written, error, critical_counter] {
                assert_eq!(
                    row(&unchanged, label)
                        .split('│')
                        .nth(3)
                        .expect("change column")
                        .trim(),
                    "-"
                );
            }
            assert_eq!(status(&unchanged, error), "—");
            assert!(status(&unchanged, critical_counter).contains(critical));
            assert!(!unchanged.contains("±0"));
        }
    }
}

#[test]
fn multiple_protocols_share_borders_and_columns_without_a_footer_even_with_color() {
    use tabled::grid::util::string::get_text_width;

    let device = Device::new(ata(json!([{"id":12,"name":"Power_Cycle_Count","raw":{"value":1}}])));
    device.write("scan", "/dev/sda -d sat\n/dev/sdb -d nvme\n");
    device.write(
        "second.json",
        &json!({
            "device":{"protocol":"NVMe"},"model_name":"型號非常長".repeat(30),"serial_number":"SECOND", "nvme_version":{"string":"1.4"},
            "smart_status":{"passed":true},"nvme_smart_health_information_log":{
                "power_cycles":1,"unsafe_shutdowns":1,"available_spare":100,"available_spare_threshold":10,"percentage_used":7,"num_err_log_entries":1
            }
        })
        .to_string(),
    );
    for locale in ["C", "de_DE.UTF-8"] {
        for (columns, width) in [("54", 54), ("72", 72), ("90", 90), ("1", 54), ("invalid", 90), ("0", 90), ("200", 90)] {
            for color in [false, true] {
                let output = device
                    .command()
                    .env("LC_ALL", locale)
                    .env("COLUMNS", columns)
                    .env("CLICOLOR_FORCE", if color { "1" } else { "0" })
                    .output()
                    .expect("run CLI");
                assert!(output.status.success());
                let text = stdout(&output);
                let lines: Vec<_> = text.lines().collect();
                let second_header = lines
                    .iter()
                    .position(|line| line.contains("/dev/sdb"))
                    .expect("second device header");
                assert!(lines[second_header - 2].is_empty(), "missing blank line between devices: {text}");
                for line in text
                    .lines()
                    .filter(|line| line.starts_with(['│', '╭', '╰', '├']))
                {
                    assert_eq!(get_text_width(line), width, "{line}");
                }
                let column_positions: Vec<Vec<_>> = text
                    .lines()
                    .filter(|line| line.matches('│').count() == 5)
                    .map(|line| {
                        line.match_indices('│')
                            .enumerate()
                            .map(|(column, (index, _))| {
                                if !color && column < 4 {
                                    assert_eq!(get_text_width(&line[..index]), line[..index].chars().count(), "{line}");
                                }
                                get_text_width(&line[..index])
                            })
                            .collect()
                    })
                    .collect();
                assert!(
                    column_positions
                        .iter()
                        .all(|positions| positions == &column_positions[0]),
                    "{text}"
                );
                assert!(text.trim_end().ends_with('╯'), "unexpected footer: {text}");
                let banner = text.lines().nth(1).expect("banner");
                assert!(banner.ends_with(&format!("v{} │", env!("CARGO_PKG_VERSION"))), "{banner}");
                assert!(
                    banner
                        .trim_start_matches("│ ")
                        .starts_with(if color {
                            "\x1b["
                        } else if locale == "C" {
                            "Storage"
                        } else {
                            "Speichermedien"
                        }),
                    "{banner}"
                );
            }
        }
    }
}

#[test]
fn every_nvme_warning_bit_stays_explicit_even_when_the_overall_status_passes() {
    use prettysmart::localization::{Language, Localization};

    let device = Device::new(json!({
        "device":{"protocol":"NVMe"},"smart_status":{"passed":true},
        "nvme_smart_health_information_log":{"critical_warning":127}
    }));
    for (locale, language) in [("C", Language::EN), ("de_DE.UTF-8", Language::DE)] {
        let loc = Localization::new(language);
        let output = device
            .command()
            .env("LC_ALL", locale)
            .output()
            .expect("run localized CLI");
        assert!(output.status.success());
        let text = stdout(&output);
        assert!(row(&text, loc.smart_status()).contains(loc.status_ok()), "{text}");
        assert_eq!(current_value(&text, loc.critical_warning_label()), "127");
        assert!(status(&text, loc.critical_warning_label()).contains(loc.status_critical()));
        assert_eq!(current_value(&text, loc.unknown_warning_bits()), "0x40");
        let properties = compact(&column_text(&text, 1));
        for warning in [
            loc.nvme_warning_spare_capacity(),
            loc.nvme_warning_temperature(),
            loc.nvme_warning_reliability(),
            loc.nvme_warning_read_only(),
            loc.nvme_warning_volatile_backup(),
            loc.nvme_warning_persistent_memory(),
        ] {
            assert!(properties.contains(&compact(warning)), "missing {warning}: {text}");
        }
    }
}

#[test]
fn prepared_percentages_and_changes_use_spaces_but_vendor_raw_values_stay_untouched() {
    use prettysmart::localization::{Language, Localization};

    for protocol in ["ATA", "NVMe"] {
        for (locale, language) in [("C", Language::EN), ("de_DE.UTF-8", Language::DE)] {
            let loc = Localization::new(language);
            let mut data = ata(json!([
                {"id":245,"name":"Percent_Life_Remaining","raw":{"value":50}},
                {"id":200,"name":"Vendor_Percent","raw":{"value":7,"string":"7%"}}
            ]));
            data["model_name"] = json!("INTEL SSD");
            data["device"]["protocol"] = json!(protocol);
            data["nvme_smart_health_information_log"] = json!({"available_spare":100,"available_spare_threshold":10,"percentage_used":7});
            let device = Device::new(data.clone());
            assert!(device.run().status.success());
            data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(49);
            data["nvme_smart_health_information_log"]["available_spare"] = json!(99);
            data["nvme_smart_health_information_log"]["percentage_used"] = json!(8);
            device.health(data, 0);
            let output = device
                .command()
                .env("LC_ALL", locale)
                .output()
                .expect("run localized CLI");
            assert!(output.status.success());
            let text = stdout(&output);
            assert_eq!(
                current_value(&text, loc.drive_health()),
                if protocol == "ATA" {
                    "49 %"
                } else if locale == "C" {
                    "92 % remaining"
                } else {
                    "92 % verbleibend"
                }
            );
            assert!(row(&text, loc.drive_health()).contains("-1 %"), "{text}");
            if protocol == "NVMe" {
                let spare = format!("{} ({}: 10 %)", loc.spare_blocks(), loc.threshold());
                assert_eq!(current_value(&text, &spare), "99 %");
                assert!(row(&text, &spare).contains("-1 %"), "{text}");
                assert_eq!(current_value(&text, loc.endurance_used()), "8 %");
                assert!(row(&text, loc.endurance_used()).contains("+1 %"), "{text}");
            } else {
                assert_eq!(current_value(&text, "Vendor_Percent"), "7%");
            }
        }
    }
}

#[test]
fn overall_status_is_the_final_right_aligned_table_row_without_hiding_alerts() {
    use prettysmart::localization::{Language, Localization};

    for protocol in ["ATA", "NVMe"] {
        for (locale, language) in [("C", Language::EN), ("de_DE.UTF-8", Language::DE)] {
            let loc = Localization::new(language);
            for passed in [json!(true), json!(false), json!(null)] {
                let mut data = ata(json!([
                    {"id":5,"name":"Reallocated_Sector_Ct","raw":{"value":1}},
                    {"id":200,"name":"Vendor_Field","raw":{"value":7}}
                ]));
                data["device"]["protocol"] = json!(protocol);
                data["smart_status"]["passed"] = passed.clone();
                data["nvme_smart_health_information_log"] = json!({"media_errors":1});
                let device = Device::new(data);
                let output = device
                    .command()
                    .env("LC_ALL", locale)
                    .output()
                    .expect("run localized CLI");
                assert_eq!(output.status.success(), passed.is_boolean());
                let text = stdout(&output);
                let lines: Vec<_> = text.lines().collect();
                let footer = lines[lines.len() - 2];
                let expected = match passed.as_bool() {
                    Some(true) => loc.status_ok(),
                    Some(false) => loc.status_critical(),
                    None => loc.unavailable(),
                };
                assert!(footer.ends_with(&format!("{}: {} │", loc.smart_status(), expected)), "{text}");
                assert_eq!(footer.matches('│').count(), 2, "{text}");
                assert!(lines[lines.len() - 3].starts_with('├'), "{text}");
                assert_eq!(text.matches(loc.smart_status()).count(), 1, "{text}");
                assert!(!row(&text, "/dev/sda").contains(loc.smart_status()), "{text}");
                assert!(
                    status(&text, if protocol == "ATA" { loc.reallocated_sectors() } else { loc.media_errors() }).contains(loc.status_critical()),
                    "{text}"
                );
                if protocol == "ATA" {
                    assert_eq!(status(&text, "Vendor_Field"), "—");
                    assert!(
                        text.find("Vendor_Field")
                            .expect("vendor value")
                            < text
                                .find(loc.smart_status())
                                .expect("overall status")
                    );
                }
                if passed.is_null() {
                    assert_eq!(current_value(&text, loc.smart_data()), loc.unavailable());
                    assert_eq!(status(&text, loc.smart_data()), "—");
                }
            }
        }
    }
}
