use crate::localization::L10N;
use crate::utils::{self, StorageDevice};
use colored::{ColoredString, Colorize};
use serde_json::Value;
use std::io::IsTerminal;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use tabled::builder::Builder;
use tabled::grid::util::string::get_text_width;
use tabled::settings::{
    Alignment, Modify, Panel, Style, Width,
    object::Columns,
    style::HorizontalLine,
    themes::{BorderCorrection, Theme},
    width::Wrap,
};
use utils::get_nvme_pcie_info;

const TABLE_WIDTH: usize = 90;
// shortcut: four readable columns need at least 54 cells; revisit for narrower terminals.
static OUTPUT_WIDTH: LazyLock<usize> = LazyLock::new(|| {
    terminal_width()
        .unwrap_or(TABLE_WIDTH)
        .clamp(54, TABLE_WIDTH)
});

pub struct TableFormatter {
    builder: Builder,
    manufacturer_rows: Vec<[String; 4]>,
}

impl Default for TableFormatter {
    fn default() -> Self {
        let mut builder = Builder::default();
        builder.push_record([
            format!("{}", L10N.table_property().cyan()),
            format!("{}", L10N.table_value().cyan()),
            format!("{}", L10N.table_change().cyan()),
            format!("{}", L10N.table_status().cyan()),
        ]);
        TableFormatter {
            builder,
            manufacturer_rows: Vec::new(),
        }
    }
}

impl TableFormatter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_row(&mut self, name: &str, value: &str, change: &str, status: Option<&str>) {
        let status_text = match status {
            Some("KRITISCH") => L10N.status_critical().red().to_string(),
            Some("WARNUNG") => L10N
                .status_warning()
                .yellow()
                .to_string(),
            _ => "—".dimmed().to_string(),
        };

        let colored_value = match status {
            Some("KRITISCH") => value.red().bold().to_string(),
            Some("WARNUNG") => value.yellow().to_string(),
            Some("UNAVAILABLE" | "INFORMATION") => value.to_string(),
            _ => value.green().to_string(),
        };

        let colored_change = if change == "-" {
            change.to_string()
        } else {
            change.bright_blue().to_string()
        };

        self.builder
            .push_record([name, &colored_value, &colored_change, &status_text]);
    }

    pub fn add_manufacturer_row(&mut self, name: &str, value: &str) {
        self.manufacturer_rows
            .push([name.to_string(), value.to_string(), "-".to_string(), "—".dimmed().to_string()]);
    }

    pub fn print_table(self, device: &StorageDevice, json: &Value) {
        println!("{}", self.render(device, json, *OUTPUT_WIDTH));
    }

    fn render(mut self, device: &StorageDevice, json: &Value, width: usize) -> String {
        let manufacturer_start = self.builder.count_records();
        let has_manufacturer_rows = !self.manufacturer_rows.is_empty();
        for row in self.manufacturer_rows {
            self.builder.push_record(row);
        }
        let mut table = self.builder.build();
        // Four padded cells and five vertical borders share the same budget for every drive.
        let available = width - 13;
        let status = get_text_width(L10N.status_critical()).max(get_text_width(L10N.status_warning()));
        let change = (available / 6).max(8);
        let value = (available / 4).max(10);
        let widths = [available - status - change - value, value, change, status];
        table.with(Alignment::left());
        for (column, width) in widths.into_iter().enumerate() {
            table.with(Modify::new(Columns::one(column)).with(Width::wrap(width).keep_words(true)));
        }
        table.with(Modify::new(Columns::new(1..=2)).with(Alignment::right()));
        table.with(Modify::new(Columns::last()).with(Alignment::center()));

        let line = HorizontalLine::full('─', '┼', '├', '┤');
        let mut theme = Theme::from(Style::rounded().remove_horizontals());
        theme.insert_horizontal_line(1, line);
        if manufacturer_start > 1 || has_manufacturer_rows {
            theme.insert_horizontal_line(2, line);
        }
        if has_manufacturer_rows {
            table.with(Panel::horizontal(manufacturer_start, Wrap::wrap(L10N.manufacturer_values(), width - 4, true)));
            theme.insert_horizontal_line(manufacturer_start + 1, line);
            theme.insert_horizontal_line(manufacturer_start + 2, line);
        }
        let header_content = device_header(device, json, width - 4);
        table.with(Panel::header(header_content));
        let footer_row = table.count_rows();
        theme.insert_horizontal_line(footer_row, line);
        table
            .with(Panel::footer(Wrap::wrap(&smart_overall_status(json), width - 4, true)))
            .with(Modify::new((footer_row, 0)).with(Alignment::right()))
            .with(theme)
            .with(BorderCorrection::span())
            .with(Width::list(widths.map(|width| width + 2)))
            .to_string()
    }
}

pub fn print_header(title: &str) {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let mut builder = Builder::default();
    builder.push_record([paired_lines(&title.cyan().bold().to_string(), &version, *OUTPUT_WIDTH - 4)]);
    println!(
        "{}",
        builder
            .build()
            .with(Style::rounded().remove_horizontals())
    );
}

fn device_header(device: &StorageDevice, json: &Value, width: usize) -> String {
    let left = format!("{} ({})", device.device_path.green(), device.interface.cyan());
    let mut lines = vec![paired_lines(&left, "", width)];
    let is_nvme = json["device"]["protocol"]
        .as_str()
        .map_or_else(|| device.is_nvme(), |protocol| protocol == "NVMe");
    let version_field = if is_nvme { "nvme_version" } else { "sata_version" };
    let model = json["model_name"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let version = json[version_field]["string"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut model_text = String::new();
    if model.is_some() || version.is_some() {
        model_text.push_str(&format!("{}: {}", L10N.device_model(), model.unwrap_or_else(|| L10N.unavailable())));
        if let Some(version) = version {
            if is_nvme {
                model_text.push_str(&format!(" ({})", format!("NVMe {}", version).cyan()));
            } else {
                model_text.push_str(&format!(" ({})", version.cyan()));
            }
        }
    }
    let capacity = json["user_capacity"]["bytes"]
        .as_u64()
        .or_else(|| json["nvme_total_capacity"].as_u64())
        .map(|bytes| format!("{}: {:.2} GB", L10N.capacity(), bytes as f64 / 1e9))
        .unwrap_or_default();
    if !model_text.is_empty() || !capacity.is_empty() {
        lines.push(paired_lines(&model_text, &capacity, width));
    }
    let field = |key: &str, label: &str| {
        json[key]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("{}: {}", label, value))
            .unwrap_or_default()
    };
    let serial = field("serial_number", L10N.serial_number_label());
    let firmware = field("firmware_version", L10N.firmware());
    if !serial.is_empty() || !firmware.is_empty() {
        lines.push(paired_lines(&serial, &firmware, width));
    }
    if is_nvme
        && device.interface == "nvme"
        && let Ok((current, maximum)) = get_nvme_pcie_info(&device.short_device_name)
    {
        lines.push(Wrap::wrap(
            &format!("{} {}", L10N.transmission_mode(), pcie_link_text(&current, &maximum)),
            width,
            true,
        ));
    }

    lines.join("\n")
}

fn smart_overall_status(json: &Value) -> String {
    let passed = json["smart_status"]["passed"].as_bool();
    let status = match passed {
        Some(true) => L10N.status_ok(),
        Some(false) => L10N.status_critical(),
        None => L10N.unavailable(),
    };
    let status = format!("{}: {}", L10N.smart_status(), status);
    match passed {
        Some(true) => status.green().to_string(),
        Some(false) => status.red().bold().to_string(),
        None => status,
    }
}

fn pcie_link_text(current: &str, maximum: &str) -> ColoredString {
    let text = format!("{} (max: {})", current, maximum);
    if current != maximum { text.yellow() } else { text.normal() }
}

fn paired_lines(left: &str, right: &str, width: usize) -> String {
    let occupied = get_text_width(left) + get_text_width(right);
    if occupied < width {
        return format!("{}{}{}", left, " ".repeat(width - occupied), right);
    }
    let right_width = get_text_width(right).min(width / 2);
    let left_width = width - right_width - 1;
    let left = Wrap::wrap(left, left_width, true);
    let right = Wrap::wrap(right, right_width.max(1), true);
    let mut left = left.lines();
    let mut right = right.lines();
    let mut lines = Vec::new();
    loop {
        let (left, right) = (left.next(), right.next());
        if left.is_none() && right.is_none() {
            break;
        }
        let (left, right) = (left.unwrap_or_default(), right.unwrap_or_default());
        let padding = width.saturating_sub(get_text_width(left) + get_text_width(right));
        lines.push(format!("{}{}{}", left, " ".repeat(padding), right));
    }
    lines.join("\n")
}

fn terminal_width() -> Option<usize> {
    if let Some(width) = std::env::var("COLUMNS")
        .ok()
        .and_then(|width| width.parse().ok())
        .filter(|&width| width > 0)
    {
        return Some(width);
    }
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let tty = std::fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty")
        .arg("size")
        .stdin(tty)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
        .filter(|&width| width > 0)
}

#[cfg(test)]
mod tests {
    use super::{TableFormatter, device_header, pcie_link_text};
    use crate::localization::L10N;
    use crate::utils::StorageDevice;
    use colored::Color;
    use serde_json::json;
    use tabled::grid::util::string::get_text_width;

    fn device() -> StorageDevice {
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
            let rendered = text.to_string();
            if rendered != text.input {
                assert!(rendered.starts_with("\x1b[33m"));
            }
            assert_eq!(get_text_width(&rendered), get_text_width(&text.input));
            assert!(get_text_width(&tabled::settings::width::Wrap::wrap(&rendered, 32, true)) <= 32);
        }
    }
}
