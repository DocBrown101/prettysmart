use colored::Colorize;
use serde_json::Value;
use std::process::Command;

use prettysmart::formatter::{TableFormatter, print_header};
use prettysmart::localization::L10N;
use prettysmart::utils::{convert_lba_to_tb, find_storage_devices};

const ENDURANCE_WARN: u64 = 70;
const ENDURANCE_CRIT: u64 = 90;

fn main() {
    print_header(L10N.header_title());

    let devices = find_storage_devices().unwrap_or_else(|err| {
        eprintln!("{}", err.to_string().red());
        std::process::exit(1);
    });
    if devices.is_empty() {
        eprintln!("{}", L10N.no_devices().red());
        std::process::exit(1);
    }

    for device in devices {
        let parts: Vec<&str> = device.all_parts.split_whitespace().collect();
        let output = Command::new("smartctl")
            .args(["-i", "-H", "-A", "-j"])
            .args(&parts)
            .output()
            .unwrap_or_else(|_e| {
                eprintln!("{}", L10N.smartctl_start_error().red());
                std::process::exit(1);
            });

        let json: Value = match serde_json::from_slice(&output.stdout) {
            Ok(json) => json,
            Err(_) if !output.status.success() => {
                eprintln!("{}", L10N.smart_data_error(&device.device_path).red());
                continue;
            }
            Err(_) => {
                eprintln!("{}", L10N.json_parse_error().red());
                std::process::exit(1);
            }
        };
        let mut formatter = TableFormatter::new();

        match device.interface.as_str() {
            "nvme" => process_nvme(&json, &mut formatter),
            _ => process_sata(&json, &mut formatter),
        }

        formatter.print_table(&device, &json);
    }
}

fn process_nvme(json: &Value, formatter: &mut TableFormatter) {
    let health = &json["nvme_smart_health_information_log"];

    // Handle critical warning with bit decoding
    if let Some(warn) = health["critical_warning"].as_u64().filter(|&w| w != 0) {
        println!("{}", L10N.critical_warning(warn).red().bold());

        // Decode individual warning bits
        let mut warnings = Vec::new();
        if warn & 0x01 != 0 {
            warnings.push(L10N.nvme_warning_spare_capacity());
        }
        if warn & 0x02 != 0 {
            warnings.push(L10N.nvme_warning_temperature());
        }
        if warn & 0x04 != 0 {
            warnings.push(L10N.nvme_warning_reliability());
        }
        if warn & 0x08 != 0 {
            warnings.push(L10N.nvme_warning_read_only());
        }
        if warn & 0x10 != 0 {
            warnings.push(L10N.nvme_warning_volatile_backup());
        }

        if !warnings.is_empty() {
            println!("  → {}", warnings.join(", "));
        }
    }

    // Handle spare blocks
    if let (Some(spare), Some(spare_thresh)) = (health["available_spare"].as_u64(), health["available_spare_threshold"].as_u64()) {
        let status = if spare <= spare_thresh {
            Some("KRITISCH")
        } else if spare <= spare_thresh.saturating_add(10) {
            Some("WARNUNG")
        } else {
            None
        };
        let name = format!("{} ({}%)", L10N.spare_blocks(), spare_thresh);
        let value = format!("{}%", spare);
        formatter.add_row(&name, &value, status);
    }

    // Handle drive health
    if let Some(pct_used) = health["percentage_used"].as_u64() {
        let remaining = 100_u64.saturating_sub(pct_used);
        let status = if pct_used >= ENDURANCE_CRIT {
            Some("KRITISCH")
        } else if pct_used >= ENDURANCE_WARN {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{} {}", remaining, L10N.remaining());
        formatter.add_row(L10N.drive_health(), &value, status);
    }

    // Handle temperature - critical composite temperature time
    if let Some(raw_value) = health["critical_comp_time"].as_u64() {
        let status = if raw_value > 0 { Some("WARNUNG") } else { None };
        let value = format!("{} min", raw_value);
        formatter.add_row(L10N.critical_comp_time(), &value, status);
    }

    // Handle data units read/written
    if let Some(raw_value) = health["data_units_read"].as_u64() {
        formatter.add_row(L10N.data_read_label(), &convert_lba_to_tb(raw_value, 512000.0), None);
    }
    if let Some(raw_value) = health["data_units_written"].as_u64() {
        formatter.add_row(L10N.data_written_label(), &convert_lba_to_tb(raw_value, 512000.0), None);
    }

    // Handle power on hours
    if let Some(raw_value) = health["power_on_hours"].as_u64() {
        formatter.add_row(L10N.operating_hours_label(), &L10N.operating_hours(raw_value), None);
    }

    // Handle power cycles
    if let Some(raw_value) = health["power_cycles"].as_u64() {
        formatter.add_row(L10N.power_cycles_label(), &raw_value.to_string(), None);
    }

    // Handle media errors
    if let Some(raw_value) = health["media_errors"].as_u64() {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        formatter.add_row(L10N.media_errors(), &raw_value.to_string(), status);
    }

    // Handle error log entries
    if let Some(raw_value) = health["num_err_log_entries"].as_u64() {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        formatter.add_row(L10N.num_err_log_entries(), &raw_value.to_string(), status);
    }

    // Handle unsafe shutdowns
    if let Some(raw_value) = health["unsafe_shutdowns"].as_u64() {
        let status = if raw_value > 0 { Some("INFORMATION") } else { None };
        formatter.add_row(L10N.unsafe_shutdowns(), &raw_value.to_string(), status);
    }

    // Handle thermal throttling
    if let Some(raw_value) = health["thermal_mgmt_temp1_trans_count"].as_u64() {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        formatter.add_row(L10N.thermal_throttling(), &raw_value.to_string(), status);
    }
    if let Some(raw_value) = health["thermal_mgmt_temp1_total_time"].as_u64()
        && raw_value > 0
    {
        let value = format!("{} min", raw_value);
        formatter.add_row(L10N.overall_throttled(), &value, Some("WARNUNG"));
    }
}

fn process_sata(json: &Value, formatter: &mut TableFormatter) {
    let attrs = &json["ata_smart_attributes"]["table"];

    fn find_entry(attrs: &Value, id: u64) -> Option<&Value> {
        attrs
            .as_array()?
            .iter()
            .find(|a| a["id"].as_u64() == Some(id))
    }

    fn attr_uses_lba_units(attr: &Value) -> bool {
        attr["name"]
            .as_str()
            .map(|name| {
                let name = name.to_ascii_lowercase();
                name.contains("lba") || name.contains("sector")
            })
            .unwrap_or(false)
    }

    fn get_lba_attr(attrs: &Value, id: u64) -> Option<u64> {
        let attr = find_entry(attrs, id)?;

        if attr_uses_lba_units(attr) {
            attr["raw"]["value"].as_u64().filter(|&v| v > 0)
        } else {
            None
        }
    }

    let get_attr = |id: u64| find_entry(attrs, id).and_then(|a| a["raw"]["value"].as_u64());
    let get_attr_value = |id: u64| find_entry(attrs, id).and_then(|a| a["value"].as_u64());

    // Handle reallocated sectors
    if let Some(raw_value) = get_attr(5) {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        formatter.add_row(L10N.reallocated_sectors(), &raw_value.to_string(), status);
    }

    // Handle current pending sectors
    if let Some(raw_value) = get_attr(197) {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        formatter.add_row(L10N.pending_sectors(), &raw_value.to_string(), status);
    }

    // Handle offline uncorrectable sectors
    if let Some(raw_value) = get_attr(198) {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        formatter.add_row(L10N.offline_uncorrectable_sectors(), &raw_value.to_string(), status);
    }

    // Handle UDMA CRC errors
    if let Some(raw_value) = get_attr(199) {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        formatter.add_row(L10N.udma_crc_errors(), &raw_value.to_string(), status);
    }

    // Handle spin retry count
    if let Some(raw_value) = get_attr(10) {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        formatter.add_row(L10N.spin_retry_count(), &raw_value.to_string(), status);
    }

    // Handle operating hours
    if let Some(raw_value) = get_attr(9) {
        formatter.add_row(L10N.operating_hours_label(), &L10N.operating_hours(raw_value), None);
    }

    // Handle power cycles
    if let Some(raw_value) = get_attr(12) {
        formatter.add_row(L10N.power_cycles_label(), &raw_value.to_string(), None);
    }

    // Handle drive health remaining (wear) - ID 177
    if let Some(wear) = get_attr_value(177) {
        let status = if wear <= 10 {
            Some("KRITISCH")
        } else if wear <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", wear);
        formatter.add_row(L10N.drive_health_remaining(), &value, status);
    }

    // Handle SSD Life Left - ID 231
    if let Some(life_left) = get_attr_value(231) {
        let status = if life_left <= 10 {
            Some("KRITISCH")
        } else if life_left <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", life_left);
        formatter.add_row(L10N.ssd_life_remaining(), &value, status);
    }

    // Handle Available Reserved Space - ID 232
    if let Some(reserved) = get_attr_value(232) {
        let status = if reserved <= 10 {
            Some("KRITISCH")
        } else if reserved <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", reserved);
        formatter.add_row(L10N.reserved_capacity_available(), &value, status);
    }

    // Handle Available Reserved Space (alternative) - ID 170
    if let Some(reserved_alt) = get_attr_value(170) {
        let status = if reserved_alt <= 10 {
            Some("KRITISCH")
        } else if reserved_alt <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", reserved_alt);
        formatter.add_row(L10N.reserved_space_alt(), &value, status);
    }

    // Handle Media Wearout Indicator - ID 233
    if let Some(wearout) = get_attr_value(233) {
        let status = if wearout <= 10 {
            Some("KRITISCH")
        } else if wearout <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", wearout);
        formatter.add_row(L10N.media_wearout_indicator(), &value, status);
    }

    // Handle Wear Leveling Count - ID 173
    if let Some(wear_level) = get_attr_value(173) {
        let status = if wear_level <= 10 {
            Some("KRITISCH")
        } else if wear_level <= 30 {
            Some("WARNUNG")
        } else {
            None
        };
        let value = format!("{}%", wear_level);
        formatter.add_row(L10N.wear_leveling(), &value, status);
    }

    // Handle total LBAs written - ID 246 (preferred over 241)
    if let Some(lbas) = get_lba_attr(attrs, 246) {
        let value = convert_lba_to_tb(lbas, 512.0);
        formatter.add_row(L10N.data_written_approx_label(), &value, None);
    } else if let Some(lbas) = get_lba_attr(attrs, 241) {
        let value = convert_lba_to_tb(lbas, 512.0);
        formatter.add_row(L10N.data_written_approx_label(), &value, None);
    }

    // Handle total LBAs read - ID 242
    if let Some(lbas) = get_lba_attr(attrs, 242) {
        let value = convert_lba_to_tb(lbas, 512.0);
        formatter.add_row(L10N.data_read_total(), &value, None);
    }
}
