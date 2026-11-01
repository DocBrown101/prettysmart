use crate::metric_context::MetricContext;
use prettysmart::localization::L10N;
use prettysmart::snapshot::DeltaFormat;
use prettysmart::utils::convert_lba_to_tb;
use serde_json::Value;

pub(crate) fn process(json: &Value, metrics: &mut MetricContext<'_>) {
    let hours = json["power_on_time"]["hours"].as_u64();
    if let Some(hours) = hours {
        // The old key may contain undecoded, vendor-specific raw hours.
        metrics.add_row(
            "sata.power_on_time.hours",
            L10N.operating_hours_label(),
            &L10N.operating_hours(hours),
            hours,
            DeltaFormat::MonotonicHours,
            None,
        );
    }
    let Some(attrs) = json["ata_smart_attributes"]["table"].as_array() else {
        return;
    };
    for attr in attrs {
        let Some(id) = attr["id"].as_u64() else { continue };
        let name = attr["name"]
            .as_str()
            .unwrap_or("Unknown_Attribute");
        let raw = attr["raw"]["value"].as_u64();
        let warning_key = match (id, name) {
            (199, "UDMA_CRC_Error_Count" | "CRC_Error_Count") => Some("sata.udma_crc_errors"),
            (10, "Spin_Retry_Count") => Some("sata.spin_retry_count"),
            _ => None,
        };
        let counter_without_change = warning_key.is_some_and(|key| {
            raw.zip(metrics.previous_value(key))
                .is_none_or(|(current, previous)| current == previous)
        });
        let failure = attribute_failure(attr).filter(|&status| !counter_without_change || status == "KRITISCH");
        let label: String = name
            .chars()
            .filter(|ch| !ch.is_ascii_digit())
            .collect();
        if let Some(status) = failure {
            let title = if status == "KRITISCH" {
                format!("{}: {}", label, L10N.failed_now())
            } else {
                label.clone()
            };
            metrics
                .formatter
                .add_row(&title, &attribute_values(attr), "-", Some(status));
        }
        if id == 9 && hours.is_some() {
            continue;
        }

        let count = match (id, name) {
            (5, "Reallocated_Sector_Ct" | "Reallocated_Sector_Count") => {
                Some(("sata.reallocated_sectors", L10N.reallocated_sectors(), "KRITISCH", DeltaFormat::Count))
            }
            (197, "Current_Pending_Sector" | "Current_Pending_Sector_Ct" | "Pending_Sector_Count") => {
                Some(("sata.pending_sectors", L10N.pending_sectors(), "KRITISCH", DeltaFormat::Count))
            }
            (198, "Offline_Uncorrectable") => Some((
                "sata.offline_uncorrectable_sectors",
                L10N.offline_uncorrectable_sectors(),
                "KRITISCH",
                DeltaFormat::Count,
            )),
            (199, "UDMA_CRC_Error_Count" | "CRC_Error_Count") => Some(("sata.udma_crc_errors", L10N.udma_crc_errors(), "WARNUNG", DeltaFormat::MonotonicCount)),
            (10, "Spin_Retry_Count") => Some(("sata.spin_retry_count", L10N.spin_retry_count(), "WARNUNG", DeltaFormat::MonotonicCount)),
            (187, "Reported_Uncorrect" | "Reported_Uncorrectable_Errors" | "Uncorrectable_Error_Cnt") => Some((
                "sata.reported_uncorrectable",
                L10N.reported_uncorrectable(),
                "KRITISCH",
                DeltaFormat::MonotonicCount,
            )),
            (12, "Power_Cycle_Count") => Some(("sata.power_cycles", L10N.power_cycles_label(), "", DeltaFormat::MonotonicCount)),
            _ => None,
        };
        if let (Some((key, title, alert, format)), Some(value)) = (count, raw) {
            let status = if value > 0 && !alert.is_empty() { Some(alert) } else { failure };
            metrics.add_row(
                key,
                title,
                &value.to_string(),
                value,
                format,
                if failure == Some("KRITISCH") { failure } else { status },
            );
            continue;
        }

        if let Some(value) = remaining_percent(json, attr) {
            let status = if value <= 10 || failure == Some("KRITISCH") {
                Some("KRITISCH")
            } else if value <= 30 {
                Some("WARNUNG")
            } else {
                failure
            };
            let title = if matches!(id, 170 | 232) {
                L10N.reserved_capacity_available()
            } else {
                L10N.drive_health()
            };
            metrics.add_row(
                &format!("sata.{}.{}.remaining_percent", id, name),
                title,
                &format!("{} %", value),
                value,
                DeltaFormat::Percent,
                status,
            );
            continue;
        }
        if let (Some(multiplier), Some(value)) = (data_multiplier(json, id, name), raw) {
            let (title, status) = match name {
                "NAND_GB_Written_TLC" => (L10N.nand_written_tlc(), Some("NEUTRAL")),
                "NAND_GB_Written_SLC" => (L10N.nand_written_slc(), Some("NEUTRAL")),
                _ => (label.as_str(), None),
            };
            metrics.add_row(
                &format!("sata.{}.{}.units_{}", id, name, multiplier),
                title,
                &convert_lba_to_tb(value, multiplier as f64),
                value,
                DeltaFormat::MonotonicTb { multiplier: multiplier as f64 },
                failure.or(status),
            );
            continue;
        }
        if failure.is_none() && raw != Some(0) {
            let value = attribute_values(attr);
            if value.trim() != "0" {
                metrics
                    .formatter
                    .add_manufacturer_row(&label, &value);
            }
        }
    }
}

fn attribute_failure(attr: &Value) -> Option<&'static str> {
    let threshold_failed = match (attr["value"].as_u64(), attr["thresh"].as_u64()) {
        (Some(value), Some(threshold)) => threshold > 0 && value <= threshold,
        _ => false,
    };
    if attr["when_failed"] == "now" || threshold_failed {
        Some("KRITISCH")
    } else if attr["when_failed"] == "past" && attr["raw"]["value"].as_u64() != Some(0) {
        Some("WARNUNG")
    } else {
        None
    }
}

fn attribute_values(attr: &Value) -> String {
    attr["raw"]["string"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            attr["raw"]["value"]
                .as_u64()
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| L10N.unavailable().to_string())
}

fn remaining_percent(json: &Value, attr: &Value) -> Option<u64> {
    let id = attr["id"].as_u64()?;
    let name = attr["name"].as_str()?;
    let model = json["model_name"]
        .as_str()
        .unwrap_or_default();
    let family = json["model_family"]
        .as_str()
        .unwrap_or_default();
    let intel = model.starts_with("INTEL ") || model.starts_with("SOLIDIGM ") || family.contains("Intel") || family.contains("Solidigm");
    // shortcut: percentage encodings are vendor-specific, even with the same name; extend only with documented profiles.
    let known = match (id, name) {
        (202, "Percent_Lifetime_Remain" | "Percent_Lifetime_Used") => family.contains("Crucial") || family.contains("Micron"),
        (169, "Remaining_Lifetime_Perc") => model.starts_with("KINGSTON "),
        (245, "Percent_Life_Remaining") | (233, "Media_Wearout_Indicator") | (170 | 232, "Available_Reservd_Space") => intel,
        (231, "SSD_Life_Left") => family.contains("SandForce"),
        (177, "Wear_Leveling_Count") => samsung_ssd(json),
        _ => false,
    };
    if known {
        // Intel attribute 245 reports remaining percent in raw, not normalized (Dell KB 000215459).
        let value = if id == 245 { &attr["raw"]["value"] } else { &attr["value"] };
        value
            .as_u64()
            .filter(|&value| value <= 100)
    } else {
        None
    }
}

fn samsung_ssd(json: &Value) -> bool {
    json["model_name"]
        .as_str()
        .is_some_and(|model| {
            model
                .to_ascii_lowercase()
                .starts_with("samsung ssd")
        })
}

fn data_multiplier(json: &Value, id: u64, name: &str) -> Option<u64> {
    match name {
        "Host_Writes_32MiB" | "Host_Reads_32MiB" => Some(32 * 1024 * 1024),
        "Host_Writes_GiB" | "Host_Reads_GiB" | "Lifetime_Writes_GiB" | "Lifetime_Reads_GiB" | "NAND_GiB_Written" | "Flash_Writes_GiB" => {
            Some(1024 * 1024 * 1024)
        }
        "Host_Writes_MiB" | "Host_Reads_MiB" => Some(1024 * 1024),
        "NAND_GB_Written_TLC" | "NAND_GB_Written_SLC" => Some(1_000_000_000),
        "Total_LBAs_Written" | "Total_LBAs_Read" => {
            let micron = json["model_family"]
                .as_str()
                .is_some_and(|family| family.contains("Crucial") || family.contains("Micron"));
            // shortcut: LBA units are only verified for these families; add others with documented units.
            if (samsung_ssd(json) && matches!(id, 241 | 242)) || (micron && id == 246 && name == "Total_LBAs_Written") {
                Some(512)
            } else {
                None
            }
        }
        _ => None,
    }
}
