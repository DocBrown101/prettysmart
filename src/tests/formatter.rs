use super::{TableFormatter, device_header, pcie_link_text};
use crate::localization::L10N;
use crate::utils::StorageDevice;
use colored::Color;
use serde_json::json;
use tabled::grid::util::string::get_text_width;

fn device() -> StorageDevice {
    // Layout assertions must not inherit forced colors from the test runner.
    colored::control::set_override(false);
    StorageDevice {
        all_parts: "/dev/sda -d sat".to_string(),
        device_path: "/dev/sda".to_string(),
        short_device_name: "sda".to_string(),
        interface: "sat".to_string(),
    }
}

#[test]
fn statuses_show_alerts_or_a_centered_dash_without_implying_ok() {
    let mut formatter = TableFormatter::new();
    for status in [None, Some("OK"), Some("NEUTRAL"), Some("INFORMATION"), Some("UNAVAILABLE")] {
        formatter.add_row("Measurement", "1", "-", status);
    }
    formatter.add_row("Warning", "1", "+1", Some("WARNUNG"));
    formatter.add_row("Critical", "1", "-", Some("KRITISCH"));
    formatter.add_manufacturer_row("Vendor value", "7");
    let output = formatter.render(&device(), &json!({}), 90);
    let header = output
        .lines()
        .find(|line| line.contains(L10N.table_property()))
        .expect("column header");
    let status_cell = header
        .split('│')
        .nth(4)
        .expect("status header");
    let left_padding = status_cell.len() - status_cell.trim_start().len();
    let right_padding = status_cell.len() - status_cell.trim_end().len();
    assert!(left_padding > 1 && left_padding.abs_diff(right_padding) <= 1, "{header}");
    assert_eq!(
        output
            .lines()
            .filter(|line| line.contains("Measurement"))
            .count(),
        5
    );
    for row in output
        .lines()
        .filter(|line| line.contains("Measurement") || line.contains("Vendor value"))
    {
        assert_eq!(
            row.split('│')
                .nth(4)
                .expect("status cell")
                .trim(),
            "—"
        );
        let cells: Vec<_> = row.split('│').collect();
        assert!(cells[1].starts_with(" Measurement") || cells[1].starts_with(" Vendor value"), "{row}");
        assert!(cells[2].ends_with("1 ") || cells[2].ends_with("7 "), "{row}");
        assert!(cells[3].ends_with("- "), "{row}");
        let left_padding = cells[4].len() - cells[4].trim_start().len();
        let right_padding = cells[4].len() - cells[4].trim_end().len();
        assert!(left_padding > 1 && left_padding.abs_diff(right_padding) <= 1, "{row}");
    }
    assert!(
        output
            .lines()
            .any(|line| line.contains("Warning") && line.contains(L10N.status_warning()))
    );
    assert!(
        output
            .lines()
            .any(|line| line.contains("Critical") && line.contains(L10N.status_critical()))
    );
}

#[test]
fn long_unicode_and_ansi_contents_keep_their_width_and_characters() {
    let name = "測定e\u{301}⚙️".repeat(25);
    let value = format!("\x1b[31m{}\x1b[0m", "數值🦀".repeat(25));
    for width in [54, 72, 90] {
        let mut formatter = TableFormatter::new();
        formatter.add_row(&name, &value, "+12345678901234567890 h", Some("KRITISCH"));
        formatter.add_manufacturer_row("Vendor_field", &"0123456789".repeat(20));
        let json = json!({"model_name": "型號".repeat(60), "serial_number": "序號".repeat(60), "firmware_version": "固件".repeat(60)});
        let output = formatter.render(&device(), &json, width);
        for line in output.lines() {
            assert_eq!(get_text_width(line), width, "{line}");
        }
        for character in ['測', '\u{301}', '⚙', '\u{fe0f}', '數', '🦀'] {
            assert_eq!(output.matches(character).count(), 25, "lost {character}: {output}");
        }
        for character in ['型', '序', '固'] {
            assert_eq!(output.matches(character).count(), 60, "lost {character}: {output}");
        }
        let raw_value: String = output
            .lines()
            .skip_while(|line| !line.contains("Vendor_field"))
            .filter_map(|line| line.split('│').nth(2))
            .map(str::trim)
            .collect();
        assert_eq!(raw_value, "0123456789".repeat(20));
        assert!(!output.contains('�'), "{output}");
    }
}

#[test]
fn absent_device_details_are_not_invented() {
    for json in [
        json!({}),
        json!({"model_name":" ","sata_version":{"string":" "},"serial_number":"","firmware_version":" "}),
    ] {
        let header = device_header(&device(), &json, 86);
        assert!(!header.contains(L10N.smart_status()));
        for label in [
            L10N.device_model(),
            L10N.capacity(),
            L10N.firmware(),
            L10N.serial_number_label(),
            L10N.transmission_mode(),
        ] {
            assert!(!header.contains(label), "{header}");
        }
    }
}

#[test]
fn serial_number_and_firmware_share_one_header_line() {
    let header = device_header(&device(), &json!({"serial_number":" TEST ","firmware_version":" FW1 "}), 86);
    let line = header
        .lines()
        .find(|line| line.contains(L10N.serial_number_label()))
        .expect("serial number line");
    assert!(line.starts_with(&format!("{}: TEST", L10N.serial_number_label())), "{header}");
    assert!(line.ends_with(&format!("{}: FW1", L10N.firmware())), "{header}");
}

#[test]
fn empty_measurements_still_have_a_closed_outer_frame() {
    let output = TableFormatter::new().render(&device(), &json!({}), 90);
    let bottom = output
        .lines()
        .last()
        .expect("bottom border");
    assert!(bottom.starts_with('╰') && bottom.ends_with('╯'), "{output}");
    let footer = output
        .lines()
        .nth_back(1)
        .expect("overall status row");
    assert!(footer.contains(L10N.smart_status()) && footer.contains(L10N.unavailable()), "{output}");
}

#[test]
fn pcie_link_is_yellow_only_when_generation_or_lane_width_differs() {
    for (current, maximum, color) in [
        ("PCIe 3.0 x4", "PCIe 3.0 x4", None),
        ("PCIe 3.0 x4", "PCIe 4.0 x4", Some(Color::Yellow)),
        ("PCIe 3.0 x2", "PCIe 3.0 x4", Some(Color::Yellow)),
    ] {
        let text = pcie_link_text(current, maximum);
        assert_eq!(text.fgcolor, color);
        assert_eq!(text.input, format!("{} (max: {})", current, maximum));
    }
}
