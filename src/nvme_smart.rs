use crate::metric_context::MetricContext;
use prettysmart::localization::L10N;
use prettysmart::snapshot::DeltaFormat;
use prettysmart::utils::convert_lba_to_tb;
use serde_json::Value;

const ENDURANCE_WARN: u64 = 70;
const ENDURANCE_CRIT: u64 = 90;

pub(crate) fn add_metrics(json: &Value, metrics: &mut MetricContext<'_>) {
    let health = &json["nvme_smart_health_information_log"];

    // Handle critical warning with bit decoding
    if let Some(warn) = health["critical_warning"]
        .as_u64()
        .filter(|&w| w != 0)
    {
        metrics
            .formatter
            .add_row(L10N.critical_warning_label(), &warn.to_string(), "-", Some("KRITISCH"));

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
        if warn & 0x20 != 0 {
            warnings.push(L10N.nvme_warning_persistent_memory());
        }
        if warn & !0x3f != 0 {
            metrics
                .formatter
                .add_row(L10N.unknown_warning_bits(), &format!("{:#x}", warn & !0x3f), "-", Some("KRITISCH"));
        }

        for warning in warnings {
            metrics
                .formatter
                .add_row(warning, "", "-", Some("KRITISCH"));
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
        let name = format!("{} ({}: {} %)", L10N.spare_blocks(), L10N.threshold(), spare_thresh);
        let value = format!("{} %", spare);
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
        metrics.add_row(
            "nvme.percentage_used",
            L10N.endurance_used(),
            &format!("{} %", pct_used),
            pct_used,
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

    for (raw_value, key, name, alert) in [
        (health["power_cycles"].as_u64(), "nvme.power_cycles", L10N.power_cycles_label(), None),
        (health["media_errors"].as_u64(), "nvme.media_errors", L10N.media_errors(), Some("KRITISCH")),
        (
            health["num_err_log_entries"].as_u64(),
            "nvme.num_err_log_entries",
            L10N.num_err_log_entries(),
            Some("WARNUNG"),
        ),
        (
            health["unsafe_shutdowns"].as_u64(),
            "nvme.unsafe_shutdowns",
            L10N.unsafe_shutdowns(),
            Some("INFORMATION"),
        ),
        (
            health["thermal_mgmt_temp1_trans_count"].as_u64(),
            "nvme.thermal_mgmt_temp1_trans_count",
            L10N.thermal_throttling(),
            Some("WARNUNG"),
        ),
    ] {
        if let Some(raw_value) = raw_value {
            let status = alert.filter(|_| raw_value > 0);
            metrics.add_row(key, name, &raw_value.to_string(), raw_value, DeltaFormat::MonotonicCount, status);
        }
    }

    // Handle thermal throttling duration
    if let Some(raw_value) = health["thermal_mgmt_temp1_total_time"].as_u64() {
        let value = format!("{} s", raw_value);
        metrics.add_row(
            "nvme.thermal_mgmt_temp1_total_time",
            L10N.overall_throttled(),
            &value,
            raw_value,
            DeltaFormat::MonotonicSeconds,
            (raw_value > 0).then_some("WARNUNG"),
        );
    }
}
