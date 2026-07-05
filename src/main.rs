use colored::Colorize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;

use prettysmart::formatter::{TableFormatter, print_header};
use prettysmart::localization::L10N;
use prettysmart::snapshot::{DeltaFormat, SnapshotStore, format_change, serial_number};
use prettysmart::utils::{convert_lba_to_tb, find_storage_devices};

const ENDURANCE_WARN: u64 = 70;
const ENDURANCE_CRIT: u64 = 90;

fn main() {
    print_header(L10N.header_title());

    let (mut snapshot_store, save_snapshots) = match SnapshotStore::load() {
        Ok(store) => (store, true),
        Err(err) => {
            eprintln!("{}", L10N.snapshot_load_error(&err.to_string()).yellow());
            (SnapshotStore::default(), false)
        }
    };

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
        let serial_number = serial_number(&json).map(str::to_string);
        let previous_metrics = serial_number
            .as_deref()
            .and_then(|serial| snapshot_store.latest_metrics(serial).cloned());
        let mut current_metrics = BTreeMap::new();

        {
            let mut metrics = MetricContext::new(&mut formatter, previous_metrics.as_ref(), &mut current_metrics);

            match device.interface.as_str() {
                "nvme" => process_nvme(&json, &mut metrics),
                _ => process_sata(&json, &mut metrics),
            }
        }

        formatter.print_table(&device, &json);

        if let Some(serial_number) = serial_number {
            snapshot_store.append(&serial_number, current_metrics);
        }
    }

    if save_snapshots && let Err(err) = snapshot_store.save() {
        eprintln!("{}", L10N.snapshot_save_error(&err.to_string()).yellow());
    }
}

fn process_nvme(json: &Value, metrics: &mut MetricContext<'_>) {
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
        metrics.add_row("nvme.available_spare", &name, &value, spare, DeltaFormat::Percent, status);
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
        metrics.add_row(
            "nvme.drive_health_remaining",
            L10N.drive_health(),
            &value,
            remaining,
            DeltaFormat::Percent,
            status,
        );
    }

    // Handle temperature - critical composite temperature time
    if let Some(raw_value) = health["critical_comp_time"].as_u64() {
        let status = if raw_value > 0 { Some("WARNUNG") } else { None };
        let value = format!("{} min", raw_value);
        metrics.add_row(
            "nvme.critical_comp_time",
            L10N.critical_comp_time(),
            &value,
            raw_value,
            DeltaFormat::MonotonicMinutes,
            status,
        );
    }

    // Handle data units read/written
    if let Some(raw_value) = health["data_units_read"].as_u64() {
        metrics.add_row(
            "nvme.data_units_read",
            L10N.data_read_label(),
            &convert_lba_to_tb(raw_value, 512000.0),
            raw_value,
            DeltaFormat::MonotonicTb { multiplier: 512000.0 },
            None,
        );
    }
    if let Some(raw_value) = health["data_units_written"].as_u64() {
        metrics.add_row(
            "nvme.data_units_written",
            L10N.data_written_label(),
            &convert_lba_to_tb(raw_value, 512000.0),
            raw_value,
            DeltaFormat::MonotonicTb { multiplier: 512000.0 },
            None,
        );
    }

    // Handle power on hours
    if let Some(raw_value) = health["power_on_hours"].as_u64() {
        metrics.add_row(
            "nvme.power_on_hours",
            L10N.operating_hours_label(),
            &L10N.operating_hours(raw_value),
            raw_value,
            DeltaFormat::MonotonicHours,
            None,
        );
    }

    // Handle power cycles
    if let Some(raw_value) = health["power_cycles"].as_u64() {
        metrics.add_row(
            "nvme.power_cycles",
            L10N.power_cycles_label(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            None,
        );
    }

    // Handle media errors
    if let Some(raw_value) = health["media_errors"].as_u64() {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        metrics.add_row(
            "nvme.media_errors",
            L10N.media_errors(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle error log entries
    if let Some(raw_value) = health["num_err_log_entries"].as_u64() {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        metrics.add_row(
            "nvme.num_err_log_entries",
            L10N.num_err_log_entries(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle unsafe shutdowns
    if let Some(raw_value) = health["unsafe_shutdowns"].as_u64() {
        let status = if raw_value > 0 { Some("INFORMATION") } else { None };
        metrics.add_row(
            "nvme.unsafe_shutdowns",
            L10N.unsafe_shutdowns(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle thermal throttling
    if let Some(raw_value) = health["thermal_mgmt_temp1_trans_count"].as_u64() {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        metrics.add_row(
            "nvme.thermal_mgmt_temp1_trans_count",
            L10N.thermal_throttling(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }
    if let Some(raw_value) = health["thermal_mgmt_temp1_total_time"].as_u64()
        && raw_value > 0
    {
        let value = format!("{} min", raw_value);
        metrics.add_row(
            "nvme.thermal_mgmt_temp1_total_time",
            L10N.overall_throttled(),
            &value,
            raw_value,
            DeltaFormat::MonotonicMinutes,
            Some("WARNUNG"),
        );
    }
}

fn process_sata(json: &Value, metrics: &mut MetricContext<'_>) {
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
        metrics.add_row(
            "sata.reallocated_sectors",
            L10N.reallocated_sectors(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle current pending sectors
    if let Some(raw_value) = get_attr(197) {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        metrics.add_row(
            "sata.pending_sectors",
            L10N.pending_sectors(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle offline uncorrectable sectors
    if let Some(raw_value) = get_attr(198) {
        let status = if raw_value >= 1 { Some("KRITISCH") } else { None };
        metrics.add_row(
            "sata.offline_uncorrectable_sectors",
            L10N.offline_uncorrectable_sectors(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle UDMA CRC errors
    if let Some(raw_value) = get_attr(199) {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        metrics.add_row(
            "sata.udma_crc_errors",
            L10N.udma_crc_errors(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle spin retry count
    if let Some(raw_value) = get_attr(10) {
        let status = if raw_value >= 1 { Some("WARNUNG") } else { None };
        metrics.add_row(
            "sata.spin_retry_count",
            L10N.spin_retry_count(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            status,
        );
    }

    // Handle operating hours
    if let Some(raw_value) = get_attr(9) {
        metrics.add_row(
            "sata.power_on_hours",
            L10N.operating_hours_label(),
            &L10N.operating_hours(raw_value),
            raw_value,
            DeltaFormat::MonotonicHours,
            None,
        );
    }

    // Handle power cycles
    if let Some(raw_value) = get_attr(12) {
        metrics.add_row(
            "sata.power_cycles",
            L10N.power_cycles_label(),
            &raw_value.to_string(),
            raw_value,
            DeltaFormat::MonotonicCount,
            None,
        );
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
        metrics.add_row(
            "sata.drive_health_remaining",
            L10N.drive_health_remaining(),
            &value,
            wear,
            DeltaFormat::Percent,
            status,
        );
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
        metrics.add_row(
            "sata.ssd_life_remaining",
            L10N.ssd_life_remaining(),
            &value,
            life_left,
            DeltaFormat::Percent,
            status,
        );
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
        metrics.add_row(
            "sata.reserved_capacity_available",
            L10N.reserved_capacity_available(),
            &value,
            reserved,
            DeltaFormat::Percent,
            status,
        );
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
        metrics.add_row(
            "sata.reserved_space_alt",
            L10N.reserved_space_alt(),
            &value,
            reserved_alt,
            DeltaFormat::Percent,
            status,
        );
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
        metrics.add_row(
            "sata.media_wearout_indicator",
            L10N.media_wearout_indicator(),
            &value,
            wearout,
            DeltaFormat::Percent,
            status,
        );
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
        metrics.add_row("sata.wear_leveling", L10N.wear_leveling(), &value, wear_level, DeltaFormat::Percent, status);
    }

    // Handle total LBAs written - ID 246 (preferred over 241)
    if let Some(lbas) = get_lba_attr(attrs, 246) {
        let value = convert_lba_to_tb(lbas, 512.0);
        metrics.add_row(
            "sata.data_written_lbas",
            L10N.data_written_approx_label(),
            &value,
            lbas,
            DeltaFormat::MonotonicTb { multiplier: 512.0 },
            None,
        );
    } else if let Some(lbas) = get_lba_attr(attrs, 241) {
        let value = convert_lba_to_tb(lbas, 512.0);
        metrics.add_row(
            "sata.data_written_lbas",
            L10N.data_written_approx_label(),
            &value,
            lbas,
            DeltaFormat::MonotonicTb { multiplier: 512.0 },
            None,
        );
    }

    // Handle total LBAs read - ID 242
    if let Some(lbas) = get_lba_attr(attrs, 242) {
        let value = convert_lba_to_tb(lbas, 512.0);
        metrics.add_row(
            "sata.data_read_lbas",
            L10N.data_read_total(),
            &value,
            lbas,
            DeltaFormat::MonotonicTb { multiplier: 512.0 },
            None,
        );
    }
}

struct MetricContext<'a> {
    formatter: &'a mut TableFormatter,
    previous_metrics: Option<&'a BTreeMap<String, u64>>,
    current_metrics: &'a mut BTreeMap<String, u64>,
}

impl<'a> MetricContext<'a> {
    fn new(formatter: &'a mut TableFormatter, previous_metrics: Option<&'a BTreeMap<String, u64>>, current_metrics: &'a mut BTreeMap<String, u64>) -> Self {
        Self {
            formatter,
            previous_metrics,
            current_metrics,
        }
    }

    fn add_row(&mut self, key: &str, name: &str, value: &str, current: u64, delta_format: DeltaFormat, status: Option<&str>) {
        let previous = self
            .previous_metrics
            .and_then(|metrics| metrics.get(key).copied());
        let change = format_change(previous, current, delta_format);

        self.formatter.add_row(name, value, &change, status);
        self.current_metrics.insert(key.to_string(), current);
    }
}
