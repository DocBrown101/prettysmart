use prettysmart::snapshot::{DeltaFormat, format_change, serial_number};
use serde_json::json;

#[test]
fn format_change_without_previous_value() {
    assert_eq!(format_change(None, 10, DeltaFormat::MonotonicCount), "-");
}

#[test]
fn format_change_without_difference() {
    assert_eq!(format_change(Some(10), 10, DeltaFormat::MonotonicCount), "±0");
}

#[test]
fn format_change_for_monotonic_hours() {
    assert_eq!(format_change(Some(100), 106, DeltaFormat::MonotonicHours), "+6 h");
}

#[test]
fn format_change_for_decreasing_percent() {
    assert_eq!(format_change(Some(95), 94, DeltaFormat::Percent), "-1%");
}

#[test]
fn format_change_for_tb_values() {
    assert_eq!(
        format_change(Some(1_953_125), 2_148_438, DeltaFormat::MonotonicTb { multiplier: 512000.0 },),
        "+100.00 GB"
    );
}

#[test]
fn format_change_for_small_data_values() {
    assert_eq!(
        format_change(Some(1_953_125), 1_953_126, DeltaFormat::MonotonicTb { multiplier: 512000.0 },),
        "+512.00 KB"
    );
}

#[test]
fn format_change_for_monotonic_reset() {
    assert_eq!(format_change(Some(100), 90, DeltaFormat::MonotonicCount), "reset?");
}

#[test]
fn serial_number_reads_non_empty_value() {
    let json = json!({ "serial_number": " ABC123 " });

    assert_eq!(serial_number(&json), Some("ABC123"));
}

#[test]
fn serial_number_ignores_missing_or_empty_values() {
    assert_eq!(serial_number(&json!({})), None);
    assert_eq!(serial_number(&json!({ "serial_number": " " })), None);
}
