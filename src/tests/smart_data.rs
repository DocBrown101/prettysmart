use crate::{ata_smart, metric_context::MetricContext, nvme_smart};
use prettysmart::formatter::TableFormatter;
use prettysmart::localization::L10N;
use prettysmart::utils::StorageDevice;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn ata(attrs: Value) -> Value {
    json!({
        "device":{"protocol":"ATA"}, "model_name":"Test SSD", "smart_status":{"passed":true},
        "power_on_time":{"hours":200}, "ata_smart_attributes":{"table":attrs}
    })
}

fn report(data: &Value, previous: Option<&BTreeMap<String, u64>>) -> (String, BTreeMap<String, u64>) {
    colored::control::set_override(false);
    let mut formatter = TableFormatter::new();
    let mut current = BTreeMap::new();
    let mut metrics = MetricContext::new(&mut formatter, previous, &mut current);
    if data["device"]["protocol"] == "NVMe" {
        nvme_smart::add_metrics(data, &mut metrics);
    } else {
        ata_smart::add_metrics(data, &mut metrics);
    }
    let device = StorageDevice {
        all_parts: "/dev/sda -d sat".to_string(),
        device_path: "/dev/sda".to_string(),
        short_device_name: "sda".to_string(),
        interface: "sat".to_string(),
    };
    (formatter.render(&device, data, 90), current)
}

fn row<'a>(text: &'a str, label: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label}: {text}"))
}

fn cell<'a>(text: &'a str, label: &str, column: usize) -> &'a str {
    row(text, label)
        .split('│')
        .nth(column)
        .expect("table column")
        .trim()
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[test]
fn undecoded_hours_are_not_guessed() {
    let mut data = ata(json!([{"id":9,"name":"Power_On_Hours_and_Msec","raw":{"value":4294967301_u64,"string":"5h+00m+01s"}}]));
    data.as_object_mut()
        .expect("object")
        .remove("power_on_time");
    let (text, metrics) = report(&data, None);
    assert!(!text.contains(L10N.operating_hours_label()), "{text}");
    assert!(row(&text, "Power_On_Hours_and_Msec").contains("5h+00m+01s"));
    assert!(!metrics.contains_key("sata.power_on_time.hours"));
}

#[test]
fn sata_names_units_and_decreasing_counts_are_respected() {
    let data = ata(json!([
        {"id":9,"name":"Power_On_Seconds","raw":{"value":720000}},
        {"id":197,"name":"Current_Pending_Sector","raw":{"value":3}},
        {"id":198,"name":"Offline_Uncorrectable","raw":{"value":1}},
        {"id":177,"name":"Read_Retry_Cnt","value":200,"raw":{"value":8}},
        {"id":233,"name":"NAND_GiB_Written","value":100,"raw":{"value":1024}},
        {"id":241,"name":"Host_Writes_32MiB","raw":{"value":0}},
        {"id":242,"name":"Lifetime_Reads_GiB","raw":{"value":1024}},
        {"id":246,"name":"Total_LBAs_Written","raw":{"value":1000000000}}
    ]));
    let previous = BTreeMap::from([
        ("sata.power_on_hours".to_string(), 720000),
        ("sata.pending_sectors".to_string(), 8),
        ("sata.offline_uncorrectable_sectors".to_string(), 2),
    ]);
    let (text, metrics) = report(&data, Some(&previous));
    assert_eq!(cell(&text, L10N.operating_hours_label(), 2), L10N.operating_hours(200));
    assert_eq!(cell(&text, L10N.operating_hours_label(), 3), "-");
    assert_eq!(cell(&text, L10N.pending_sectors(), 3), "-5");
    assert_eq!(cell(&text, L10N.offline_uncorrectable_sectors(), 3), "-1");
    assert!(!row(&text, "Read_Retry_Cnt").contains('%'));
    assert!(row(&text, "NAND_GiB_Written").contains("1.10 TB"));
    assert!(row(&text, "Lifetime_Reads_GiB").contains("1.10 TB"));
    assert_eq!(cell(&text, "Total_LBAs_Written", 2), "1000000000");
    assert_eq!(metrics["sata.power_on_time.hours"], 200);
    assert_eq!(metrics["sata.241.Host_Writes_32MiB.units_33554432"], 0);
}

#[test]
fn ata_zero_values_do_not_show_past_failures() {
    let data = ata(json!([
        {"id":1,"name":"Raw_Read_Error_Rate","value":100,"thresh":10,"when_failed":"past","raw":{"value":0}},
        {"id":10,"name":"Spin_Retry_Count","value":100,"thresh":10,"when_failed":"past","raw":{"value":0}},
        {"id":202,"name":"Program_Fail_Cnt_Total","raw":{"string":"0"}}
    ]));
    let (text, _) = report(&data, None);
    for label in ["Raw_Read_Error_Rate", "Spin Retry Count", "Program_Fail_Cnt_Total", L10N.status_warning()] {
        assert!(!text.contains(label), "{text}");
    }
}

#[test]
fn ata_threshold_and_past_failures_keep_their_severity() {
    let data = ata(json!([
        {"id":1,"name":"Raw_Read_Error_Rate","value":5,"thresh":10,"when_failed":"now","raw":{"value":0}},
        {"id":184,"name":"End-to-End_Error","value":99,"thresh":90,"when_failed":"past","raw":{"value":1}},
        {"id":187,"name":"Reported_Uncorrect","raw":{"value":2}}
    ]));
    let (text, _) = report(&data, None);
    assert!(row(&text, "Raw_Read_Error_Rate:").contains(L10N.status_critical()));
    assert!(row(&text, "End-to-End_Error").contains(L10N.status_warning()));
    assert!(row(&text, L10N.reported_uncorrectable()).contains(L10N.status_critical()));
    assert!(!text.contains("Threshold failed in the past"));
    assert!(!text.contains("Grenzwert früher verletzt"));
}

#[test]
fn ata_past_failures_warn_but_healthy_endurance_stays_neutral() {
    let mut data = ata(json!([
        {"id":184,"name":"End-to-End_Error","value":99,"thresh":90,"when_failed":"past","raw":{"value":1}},
        {"id":200,"name":"Unknown_Attribute","when_failed":"past"},
        {"id":233,"name":"Media_Wearout_Indicator","value":99,"when_failed":"past","raw":{"value":0}}
    ]));
    data["model_name"] = json!("INTEL SSDSC2KB480G8");
    let (text, _) = report(&data, None);
    assert_eq!(cell(&text, L10N.drive_health(), 2), "99 %");
    assert_eq!(cell(&text, L10N.drive_health(), 4), "—");
    for label in ["End-to-End_Error", "Unknown_Attribute"] {
        assert!(cell(&text, label, 4).contains(L10N.status_warning()), "{text}");
    }
}

#[test]
fn ata_property_labels_omit_attribute_ids_and_digits() {
    let (text, _) = report(
        &ata(json!([
            {"id":201,"name":"Vendor_Attribute_123","raw":{"value":7}},
            {"id":241,"name":"Host_Writes_32MiB","raw":{"value":1}}
        ])),
        None,
    );
    assert_eq!(cell(&text, "Vendor_Attribute_", 2), "7");
    assert_eq!(cell(&text, "Host_Writes_MiB", 2), "0.00 TB");
    for line in text
        .lines()
        .filter(|line| line.matches('│').count() == 5)
    {
        assert!(
            !line
                .split('│')
                .nth(1)
                .expect("property")
                .chars()
                .any(|ch| ch.is_ascii_digit()),
            "{line}"
        );
    }
}

#[test]
fn nand_writes_use_decimal_gb_and_stay_neutral() {
    let mut data = ata(json!([
        {"id":233,"name":"NAND_GB_Written_TLC","raw":{"value":0}},
        {"id":234,"name":"NAND_GB_Written_SLC","raw":{"value":0}}
    ]));
    let (text, zero) = report(&data, None);
    assert!(!text.contains("NAND"));
    data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(6805);
    data["ata_smart_attributes"]["table"][1]["raw"]["value"] = json!(15295);
    let (text, previous) = report(&data, Some(&zero));
    for (label, value) in [(L10N.nand_written_tlc(), "6.80 TB"), (L10N.nand_written_slc(), "15.29 TB")] {
        assert_eq!(cell(&text, label, 2), value);
        assert_eq!(cell(&text, label, 4), "—");
    }
    assert_eq!(previous["sata.233.NAND_GB_Written_TLC.units_1000000000"], 6805);
    assert_eq!(previous["sata.234.NAND_GB_Written_SLC.units_1000000000"], 15295);
    data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(6806);
    data["ata_smart_attributes"]["table"][1]["raw"]["value"] = json!(15297);
    let (text, _) = report(&data, Some(&previous));
    assert_eq!(cell(&text, L10N.nand_written_tlc(), 3), "+1.00 GB");
    assert_eq!(cell(&text, L10N.nand_written_slc(), 3), "+2.00 GB");
    assert!(!text.contains(L10N.status_warning()));
    assert!(!text.contains(L10N.status_critical()));
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
        let (text, _) = report(&data, None);
        assert_eq!(cell(&text, L10N.nand_written_tlc(), 2), "6.80 TB");
        assert_eq!(cell(&text, L10N.nand_written_slc(), 2), "15.29 TB");
        assert!(!text.contains("NAND_GB_Written"));
    }
}

#[test]
fn nand_threshold_failures_remain_visible_and_critical_even_at_zero() {
    let (text, _) = report(
        &ata(json!([{"id":233,"name":"NAND_GB_Written_TLC","value":5,"thresh":10,"raw":{"value":0}}])),
        None,
    );
    assert!(row(&text, L10N.nand_written_tlc()).contains(L10N.status_critical()));
}

#[test]
fn recognized_remaining_endurance_uses_profile_specific_values() {
    for (model, family, attr, expected, severity) in [
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
            L10N.status_warning(),
        ),
        (
            "INTEL SSDSC2KB480G8",
            "Intel S4x00",
            json!({"id":245,"name":"Percent_Life_Remaining","value":64,"raw":{"value":10}}),
            10,
            L10N.status_critical(),
        ),
        (
            "SSDSC2KB480G7R",
            "Dell Certified Intel S4x00/D3-S4x10 Series SSDs",
            json!({"id":245,"name":"Percent_Life_Remaining","value":82,"raw":{"value":5}}),
            5,
            L10N.status_critical(),
        ),
    ] {
        let mut data = ata(json!([attr]));
        data["model_name"] = json!(model);
        data["model_family"] = json!(family);
        let (text, metrics) = report(&data, None);
        assert_eq!(cell(&text, L10N.drive_health(), 2), format!("{expected} %"));
        assert!(cell(&text, L10N.drive_health(), 4).ends_with(severity), "{text}");
        let key = format!("sata.{}.{}.remaining_percent", attr["id"], attr["name"].as_str().expect("name"));
        assert_eq!(metrics[&key], expected);
        let (unchanged, _) = report(&data, Some(&metrics));
        assert_eq!(cell(&unchanged, L10N.drive_health(), 4), cell(&text, L10N.drive_health(), 4));
    }
}

#[test]
fn intel_remaining_endurance_validates_raw_percentage_without_normalized_fallback() {
    for (raw, expected) in [(json!(0), Some(0)), (json!(100), Some(100)), (json!(101), None), (json!(null), None)] {
        let mut data = ata(json!([{"id":245,"name":"Percent_Life_Remaining","value":64,"raw":{"value":raw}}]));
        data["model_name"] = json!("INTEL SSDSC2KB480G8");
        let (text, _) = report(&data, None);
        if let Some(percent) = expected {
            assert_eq!(cell(&text, L10N.drive_health(), 2), format!("{percent} %"));
        } else {
            assert!(!text.contains(L10N.drive_health()));
            assert_eq!(cell(&text, "Percent_Life_Remaining", 2), if raw.is_null() { L10N.unavailable() } else { "101" });
        }
    }
}

#[test]
fn unknown_endurance_encodings_remain_raw_vendor_values() {
    let (text, _) = report(
        &ata(json!([
            {"id":170,"name":"Grown_Failing_Block_Ct","value":100,"raw":{"value":3}},
            {"id":231,"name":"SSD_Life_Left","value":4,"raw":{"value":96}},
            {"id":233,"name":"Media_Wearout_Indicator","value":100,"raw":{"value":7}}
        ])),
        None,
    );
    for (name, value) in [("Grown_Failing_Block_Ct", "3"), ("SSD_Life_Left", "96"), ("Media_Wearout_Indicator", "7")] {
        assert_eq!(cell(&text, name, 2), value);
        assert!(!row(&text, name).contains('%'));
    }
}

#[test]
fn intel_error_counter_aliases_warn_and_keep_tracking_changes() {
    let data = ata(json!([
        {"id":197,"name":"Pending_Sector_Count","raw":{"value":2}},
        {"id":199,"name":"CRC_Error_Count","raw":{"value":3}}
    ]));
    let previous = BTreeMap::from([("sata.pending_sectors".to_string(), 1), ("sata.udma_crc_errors".to_string(), 1)]);
    let (text, metrics) = report(&data, Some(&previous));
    assert!(row(&text, L10N.pending_sectors()).contains(L10N.status_critical()));
    assert_eq!(cell(&text, L10N.pending_sectors(), 3), "+1");
    assert!(row(&text, L10N.udma_crc_errors()).contains(L10N.status_warning()));
    assert_eq!(cell(&text, L10N.udma_crc_errors(), 3), "+2");
    assert_eq!(metrics["sata.pending_sectors"], 2);
    assert_eq!(metrics["sata.udma_crc_errors"], 3);
}

#[test]
fn ata_counter_warnings_require_a_changed_positive_count() {
    for (id, name, key, label) in [
        (199, "UDMA_CRC_Error_Count", "sata.udma_crc_errors", L10N.udma_crc_errors()),
        (199, "CRC_Error_Count", "sata.udma_crc_errors", L10N.udma_crc_errors()),
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
            let data = ata(json!([{"id":id,"name":name,"value":100,"thresh":0,"when_failed":"past","raw":{"value":current}}]));
            let previous = previous.map(|value| BTreeMap::from([(key.to_string(), value)]));
            let (text, metrics) = report(&data, previous.as_ref());
            assert_eq!(text.contains(L10N.status_warning()), warning, "{name}: {text}");
            assert_eq!(text.contains(label), current > 0, "{text}");
            let (unchanged, _) = report(&data, Some(&metrics));
            assert!(!unchanged.contains(L10N.status_warning()), "{unchanged}");
        }
    }
}

#[test]
fn hidden_ata_zero_values_stay_in_metrics() {
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
    let (text, metrics) = report(&data, None);
    assert!(!text.contains(L10N.operating_hours_label()));
    assert!(!text.contains("Used_Rsvd_Blk_Cnt_Tot"));
    for key in [
        "sata.reallocated_sectors",
        "sata.power_cycles",
        "sata.reported_uncorrectable",
        "sata.pending_sectors",
        "sata.offline_uncorrectable_sectors",
        "sata.udma_crc_errors",
        "sata.241.Host_Writes_GiB.units_1073741824",
        "sata.power_on_time.hours",
    ] {
        assert_eq!(metrics[key], 0, "{key}");
    }
}

#[test]
fn unchanged_udma_crc_threshold_failures_remain_critical() {
    let data = ata(json!([{"id":199,"name":"UDMA_CRC_Error_Count","value":5,"thresh":10,"raw":{"value":3}}]));
    let previous = BTreeMap::from([("sata.udma_crc_errors".to_string(), 3)]);
    let (text, _) = report(&data, Some(&previous));
    assert!(row(&text, L10N.udma_crc_errors()).contains(L10N.status_critical()));
}

#[test]
fn nvme_zero_values_are_hidden_except_for_threshold_failures_and_counters_warn_on_change() {
    let mut data = json!({"device":{"protocol":"NVMe"},"smart_status":{"passed":true},"nvme_smart_health_information_log":{
        "available_spare":0,"available_spare_threshold":10,"percentage_used":100,"power_on_hours":0,
        "power_cycles":0,"data_units_written":0,"media_errors":0,"num_err_log_entries":0,
        "critical_comp_time":0,"thermal_mgmt_temp1_total_time":0
    }});
    let mut previous = None;
    for (current, warning) in [(0, false), (3, true), (3, false)] {
        data["nvme_smart_health_information_log"]["num_err_log_entries"] = json!(current);
        let (text, metrics) = report(&data, previous.as_ref());
        assert!(row(&text, L10N.spare_blocks()).contains(L10N.status_critical()));
        assert!(row(&text, L10N.drive_health()).contains(L10N.status_critical()));
        for label in [
            L10N.operating_hours_label(),
            L10N.power_cycles_label(),
            L10N.data_written_label(),
            L10N.media_errors(),
            L10N.critical_comp_time(),
            L10N.overall_throttled(),
        ] {
            assert!(!text.contains(label), "{text}");
        }
        assert_eq!(text.contains(L10N.num_err_log_entries()), current > 0, "{text}");
        assert_eq!(text.contains(L10N.status_warning()), warning, "{text}");
        assert_eq!(metrics["nvme.thermal_mgmt_temp1_total_time"], 0);
        previous = Some(metrics);
    }
}

#[test]
fn nvme_wear_above_100_keeps_original_usage_and_zero_remaining() {
    let data = json!({"device":{"protocol":"NVMe"},"smart_status":{"passed":false},"nvme_smart_health_information_log":{"percentage_used":150}});
    let (text, metrics) = report(&data, None);
    assert_eq!(cell(&text, L10N.endurance_used(), 2), "150 %");
    assert_eq!(cell(&text, L10N.drive_health(), 2), format!("0 {}", L10N.remaining()));
    for label in [L10N.endurance_used(), L10N.drive_health()] {
        assert!(cell(&text, label, 4).contains(L10N.status_critical()), "{text}");
    }
    assert_eq!(metrics["nvme.drive_health_remaining"], 0);
}

#[test]
fn nvme_metrics_do_not_use_ata_values() {
    let data = json!({
        "device":{"protocol":"NVMe"}, "smart_status":{"passed":true}, "power_on_time":{"hours":123},
        "ata_smart_attributes":{"table":[{"id":9,"raw":{"value":123}}]},
        "nvme_smart_health_information_log":{
            "data_units_written":48067959, "power_on_hours":2929, "media_errors":0,
            "critical_comp_time":2, "thermal_mgmt_temp1_total_time":120
        }
    });
    let previous = BTreeMap::from([("nvme.thermal_mgmt_temp1_total_time".to_string(), 60)]);
    let (text, metrics) = report(&data, Some(&previous));
    assert!(text.contains("24.61 TB"));
    assert_eq!(cell(&text, L10N.operating_hours_label(), 2), L10N.operating_hours(2929));
    assert_eq!(cell(&text, L10N.overall_throttled(), 2), "120 s");
    assert_eq!(cell(&text, L10N.overall_throttled(), 3), "+60 s");
    assert_eq!(cell(&text, L10N.critical_comp_time(), 2), "2 min");
    assert_eq!(metrics["nvme.power_on_hours"], 2929);
    assert_eq!(metrics["nvme.data_units_written"], 48067959);
    assert!(!metrics.contains_key("sata.power_on_time.hours"));
}

#[test]
fn zero_baselines_show_changes_without_turning_normal_metrics_into_warnings() {
    for protocol in ["ATA", "NVMe"] {
        let mut data = ata(json!([
            {"id":12,"name":"Power_Cycle_Count","raw":{"value":0}},
            {"id":199,"name":"UDMA_CRC_Error_Count","raw":{"value":0}},
            {"id":5,"name":"Reallocated_Sector_Ct","raw":{"value":0}},
            {"id":241,"name":"Host_Writes_GiB","raw":{"value":0}}
        ]));
        data["device"]["protocol"] = json!(protocol);
        data["nvme_smart_health_information_log"] = json!({
            "power_cycles":0,"data_units_written":0,"num_err_log_entries":0,"media_errors":0,"unsafe_shutdowns":0
        });
        let (_, previous) = report(&data, None);
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
        let (text, metrics) = report(&data, Some(&previous));
        let (written, error, critical) = if protocol == "ATA" {
            ("Host_Writes_GiB", L10N.udma_crc_errors(), L10N.reallocated_sectors())
        } else {
            (L10N.data_written_label(), L10N.num_err_log_entries(), L10N.media_errors())
        };
        for label in [L10N.power_cycles_label(), written] {
            assert_eq!(cell(&text, label, 4), "—");
        }
        assert_eq!(cell(&text, L10N.power_cycles_label(), 3), "+1");
        assert_eq!(cell(&text, written, 3), if protocol == "ATA" { "+1.07 GB" } else { "+512.00 KB" });
        assert!(cell(&text, error, 4).contains(L10N.status_warning()));
        assert!(cell(&text, critical, 4).contains(L10N.status_critical()));
        if protocol == "NVMe" {
            assert_eq!(cell(&text, L10N.unsafe_shutdowns(), 4), "—");
        }
        let (unchanged, _) = report(&data, Some(&metrics));
        for label in [L10N.power_cycles_label(), written, error, critical] {
            assert_eq!(cell(&unchanged, label, 3), "-");
        }
        assert_eq!(cell(&unchanged, error, 4), "—");
        assert!(cell(&unchanged, critical, 4).contains(L10N.status_critical()));
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
        let (_, previous) = report(&data, None);
        data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(1953125000_u64);
        let (text, _) = report(&data, Some(&previous));
        assert_eq!(cell(&text, "Total_LBAs_Written", 2), "1.00 TB");
        assert_eq!(cell(&text, "Total_LBAs_Written", 3), "+1.00 TB");
    }
}

#[test]
fn vendor_section_follows_known_neutral_metrics_and_never_contains_alerts() {
    let data = ata(json!([
        {"id":200,"name":"Vendor_Field","raw":{"value":9,"string":"9 (vendor encoding)"}},
        {"id":233,"name":"NAND_GB_Written_TLC","raw":{"value":6805}},
        {"id":184,"name":"End-to-End_Error","when_failed":"past","raw":{"value":1}},
        {"id":199,"name":"UDMA_CRC_Error_Count","raw":{"value":1}},
        {"id":201,"name":"Another_Vendor_Field","raw":{"value":0}},
        {"id":202,"name":"Missing_Vendor_Value"}
    ]));
    let previous = BTreeMap::from([("sata.udma_crc_errors".to_string(), 0)]);
    let (text, _) = report(&data, Some(&previous));
    let (known, raw) = text
        .split_once(L10N.manufacturer_values())
        .expect("vendor section");
    assert!(known.contains(L10N.nand_written_tlc()));
    assert_eq!(cell(&text, L10N.nand_written_tlc(), 4), "—");
    for label in ["End-to-End_Error", L10N.udma_crc_errors()] {
        assert!(cell(known, label, 4).contains(L10N.status_warning()), "{text}");
    }
    assert!(!raw.contains(L10N.status_warning()));
    assert_eq!(cell(raw, "Vendor_Field", 4), "—");
    let values: String = raw
        .lines()
        .filter_map(|line| line.split('│').nth(2))
        .collect();
    assert!(compact(&values).contains("9(vendorencoding)"), "{text}");
    assert!(!raw.contains("Another_Vendor_Field"));
    assert_eq!(cell(raw, "Missing_Vendor_Value", 2), L10N.unavailable());
}

#[test]
fn nvme_warning_bits_only_show_the_corresponding_warning() {
    let warnings = [
        (0x01, L10N.nvme_warning_spare_capacity()),
        (0x02, L10N.nvme_warning_temperature()),
        (0x04, L10N.nvme_warning_reliability()),
        (0x08, L10N.nvme_warning_read_only()),
        (0x10, L10N.nvme_warning_volatile_backup()),
        (0x20, L10N.nvme_warning_persistent_memory()),
    ];
    for bits in [0, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7f] {
        let data = json!({"device":{"protocol":"NVMe"},"smart_status":{"passed":true},"nvme_smart_health_information_log":{"critical_warning":bits}});
        let (text, _) = report(&data, None);
        let properties: String = text
            .lines()
            .filter_map(|line| line.split('│').nth(1))
            .collect();
        for (bit, warning) in warnings {
            assert_eq!(compact(&properties).contains(&compact(warning)), bits & bit != 0, "bits={bits:#x}: {text}");
        }
        assert_eq!(text.contains(L10N.critical_warning_label()), bits != 0, "{text}");
        assert_eq!(text.contains(L10N.unknown_warning_bits()), bits & !0x3f != 0, "{text}");
        assert!(row(&text, L10N.smart_status()).contains(L10N.status_ok()));
    }
}

#[test]
fn prepared_percentages_and_changes_use_spaces_but_vendor_raw_values_stay_untouched() {
    let mut data = ata(json!([
        {"id":245,"name":"Percent_Life_Remaining","raw":{"value":50}},
        {"id":200,"name":"Vendor_Percent","raw":{"value":7,"string":"7%"}}
    ]));
    data["model_name"] = json!("INTEL SSD");
    let (_, previous) = report(&data, None);
    data["ata_smart_attributes"]["table"][0]["raw"]["value"] = json!(49);
    let (text, _) = report(&data, Some(&previous));
    assert_eq!(cell(&text, L10N.drive_health(), 2), "49 %");
    assert_eq!(cell(&text, L10N.drive_health(), 3), "-1 %");
    assert_eq!(cell(&text, "Vendor_Percent", 2), "7%");
}
