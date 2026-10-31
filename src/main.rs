use colored::Colorize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;

mod ata_smart;
mod metric_context;
mod nvme_smart;

use metric_context::MetricContext;
use prettysmart::formatter::{TableFormatter, print_header};
use prettysmart::localization::L10N;
use prettysmart::snapshot::{SnapshotStore, serial_number};
use prettysmart::utils::{find_storage_devices, print_smartctl_messages, smartctl_read_failed};

fn main() {
    print_header(L10N.header_title());

    let (mut snapshot_store, save_snapshots) = match SnapshotStore::load() {
        Ok(store) => (store, true),
        Err(err) => {
            let message = L10N.snapshot_load_error(&err.to_string());
            eprintln!("{}", message.yellow());
            (SnapshotStore::default(), false)
        }
    };

    let devices = find_storage_devices().unwrap_or_else(|err| {
        let message = err.to_string();
        eprintln!("{}", message.red());
        std::process::exit(1);
    });
    if devices.is_empty() {
        let message = L10N.no_devices();
        eprintln!("{}", message.red());
        std::process::exit(1);
    }

    let mut query_failed = false;
    for device in devices {
        let output = match Command::new("smartctl")
            .args(["-i", "-H", "-A", "-j"])
            .args(device.all_parts.split_whitespace())
            .output()
        {
            Ok(output) => output,
            Err(err) => {
                eprintln!("{}: {}: {}", device.device_path, L10N.smartctl_start_error(), err);
                query_failed = true;
                continue;
            }
        };

        let json: Value = match serde_json::from_slice(&output.stdout) {
            Ok(json) => json,
            Err(_) => {
                eprintln!("{}: {}", device.device_path, L10N.json_parse_error());
                eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
                query_failed = true;
                continue;
            }
        };
        print_smartctl_messages(&json, &device.device_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            eprintln!("{}: {}", device.device_path, stderr.trim());
        }
        let mut formatter = TableFormatter::new();
        let protocol = json["device"]["protocol"]
            .as_str()
            .unwrap_or_else(|| {
                if device.is_nvme() {
                    "NVMe"
                } else if json["ata_smart_attributes"]["table"].is_array() || device.interface == "ata" || device.interface.starts_with("sat") {
                    "ATA"
                } else {
                    "unknown"
                }
            });
        if !matches!(protocol, "NVMe" | "ATA") {
            eprintln!("{}: {} ({})", device.device_path, L10N.unsupported_protocol(), protocol);
            formatter.add_row(L10N.smart_data(), L10N.unsupported_protocol(), "-", Some("UNAVAILABLE"));
            formatter.print_table(&device, &json);
            query_failed = true;
            continue;
        }
        let is_nvme = protocol == "NVMe";
        let has_data = if is_nvme {
            json["nvme_smart_health_information_log"]
                .as_object()
                .is_some_and(|health| !health.is_empty())
        } else {
            json["ata_smart_attributes"]["table"]
                .as_array()
                .is_some_and(|attrs| !attrs.is_empty())
        };
        // Health-warning bits (3–7) do not invalidate the measurements.
        let read_failed = smartctl_read_failed(&output, &json) || !has_data || !json["smart_status"]["passed"].is_boolean();
        if read_failed {
            eprintln!(
                "{}",
                L10N.smart_data_error(&device.device_path)
                    .red()
            );
            formatter.add_row(L10N.smart_data(), L10N.unavailable(), "-", Some("UNAVAILABLE"));
            query_failed = true;
        }
        let serial_number = serial_number(&json);
        let previous_metrics = serial_number.and_then(|serial| snapshot_store.latest_metrics(serial));
        let mut current_metrics = BTreeMap::new();

        {
            let mut metrics = MetricContext::new(&mut formatter, previous_metrics, &mut current_metrics);

            if is_nvme {
                nvme_smart::process(&json, &mut metrics);
            } else {
                ata_smart::process(&json, &mut metrics);
            }
        }

        formatter.print_table(&device, &json);

        if !read_failed && let Some(serial_number) = serial_number {
            snapshot_store.append(serial_number, current_metrics);
        }
    }

    if save_snapshots && let Err(err) = snapshot_store.save() {
        let message = L10N.snapshot_save_error(&err.to_string());
        eprintln!("{}", message.yellow());
    }
    if query_failed {
        std::process::exit(1);
    }
}
