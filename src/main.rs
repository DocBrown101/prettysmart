use colored::Colorize;
use serde_json::Value;
use std::process::Command;

mod formatter;
use formatter::{add_row, create_table_builder, print_header, print_table};
mod utils;
use utils::{convert_data_units, find_storage_devices};
mod local_strings;
use local_strings::LocalStrings;

const ENDURANCE_WARN: i64 = 70;
const ENDURANCE_CRIT: i64 = 90;

fn main() {
    let strings = LocalStrings::new();
    let devices = find_storage_devices();

    if devices.is_empty() {
        eprintln!("{}", strings.no_devices().red());
        std::process::exit(1);
    }

    print_header(strings.header_title());

    for device in devices {
        let parts: Vec<&str> = device.all_parts.split_whitespace().collect();

        let output_info = Command::new("smartctl")
            .args(["-i"])
            .args(&parts)
            .output()
            .expect(strings.smartctl_start_error());

        let output_json = Command::new("smartctl")
            .args(["-A", "-j"])
            .args(&parts)
            .output()
            .expect(strings.smartctl_start_error());

        if !output_info.status.success() || !output_json.status.success() {
            eprintln!("{}", strings.smart_data_error(&device.device_path).red());
            continue;
        }

        let json: Value = serde_json::from_slice(&output_json.stdout).expect(strings.json_parse_error());

        let mut builder = create_table_builder(&strings);

        match device.interface.as_str() {
            "nvme" => process_nvme(&json, &strings, &mut builder),
            _ => process_sata(&json, &strings, &mut builder),
        }

        print_table(&device, &output_info, builder);
        println!();
    }
}

fn process_nvme(json: &Value, strings: &LocalStrings, builder: &mut tabled::builder::Builder) {
    let health = &json["nvme_smart_health_information_log"];

    if let Some(warn) = health["critical_warning"].as_i64() {
        if warn != 0 {
            println!("{}", strings.critical_warning(warn).red().bold());
        }
    }

    let spare = health["available_spare"].as_i64().unwrap_or(-1);
    let spare_thresh = health["available_spare_threshold"].as_i64().unwrap_or(-1);
    if spare >= 0 {
        let status = if spare <= spare_thresh {
            Some("KRITISCH")
        } else if spare <= spare_thresh + 10 {
            Some("WARNUNG")
        } else {
            None
        };
        let name = if spare_thresh >= 0 {
            format!("{} ({}%)", strings.spare_blocks(), spare_thresh)
        } else {
            strings.spare_blocks().to_string()
        };
        let value = format!("{}%", spare);
        add_row(builder, strings, &name, &value, status);
    }

    if let Some(pct_used) = health["percentage_used"].as_i64() {
        let remaining = 100 - pct_used;
        let status = if pct_used >= ENDURANCE_CRIT {
            Some("KRITISCH")
        } else if pct_used >= ENDURANCE_WARN {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{} {}", remaining, strings.remaining());
        add_row(builder, strings, strings.drive_health(), &value, status);
    }

    if let Some(read) = health["data_units_read"].as_i64() {
        add_row(builder, strings, strings.data_read_label(), &convert_data_units(read), None);
    }
    if let Some(written) = health["data_units_written"].as_i64() {
        add_row(builder, strings, strings.data_written_label(), &convert_data_units(written), None);
    }

    if let Some(hours) = health["power_on_hours"].as_i64() {
        let value = format!("{} h ({} Tage)", hours, hours / 24);
        add_row(builder, strings, strings.operating_hours_label(), &value, None);
    }

    if let Some(cycles) = health["power_cycles"].as_i64() {
        add_row(builder, strings, strings.power_cycles_label(), &cycles.to_string(), None);
    }

    if let Some(media_errors) = health["media_errors"].as_i64() {
        let status = if media_errors >= 1 { Some("WARNUNG") } else { None };
        add_row(builder, strings, strings.media_errors(), &media_errors.to_string(), status);
    }

    if let Some(unsafe_shutdowns) = health["unsafe_shutdowns"].as_i64() {
        let status = if unsafe_shutdowns >= 10 { Some("WARNUNG") } else { None };
        add_row(builder, strings, strings.unsafe_shutdowns(), &unsafe_shutdowns.to_string(), status);
    }
}

fn process_sata(json: &Value, strings: &LocalStrings, builder: &mut tabled::builder::Builder) {
    let attrs = &json["ata_smart_attributes"]["table"];

    let get_attr = |id: i64| -> Option<i64> {
        attrs.as_array().and_then(|arr| {
            arr.iter()
                .find(|a| a["id"].as_i64() == Some(id))
                .and_then(|a| a["raw"]["value"].as_i64())
        })
    };

    if let Some(realloc) = get_attr(5) {
        let status = if realloc >= 1 { Some("WARNUNG") } else { None };
        add_row(builder, strings, strings.reallocated_sectors(), &realloc.to_string(), status);
    }

    if let Some(spin_retry) = get_attr(10) {
        let status = if spin_retry >= 1 { Some("WARNUNG") } else { None };
        add_row(builder, strings, strings.spin_retry_count(), &spin_retry.to_string(), status);
    }

    if let Some(hours) = get_attr(9) {
        let value = format!("{} h ({} Tage)", hours, hours / 24);
        add_row(builder, strings, strings.operating_hours_label(), &value, None);
    }

    if let Some(cycles) = get_attr(12) {
        add_row(builder, strings, strings.power_cycles_label(), &cycles.to_string(), None);
    }

    if let Some(wear) = attrs
        .as_array()
        .and_then(|arr| arr.iter().find(|a| a["id"].as_i64() == Some(177)))
        .and_then(|a| a["value"].as_i64())
    {
        let status = if wear <= 10 {
            Some("KRITISCH")
        } else if wear <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", wear);
        add_row(builder, strings, strings.drive_health_remaining(), &value, status);
    }

    if let Some(lbas) = get_attr(241) {
        let tb = (lbas as f64 * 512.0) / 1e12;
        let value = format!("{:.2} TB", tb);
        add_row(builder, strings, strings.data_written_approx_label(), &value, None);
    }
}
