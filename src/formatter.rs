use crate::localization::L10N;
use crate::utils::{self, StorageDevice};
use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;
use tabled::settings::{Alignment, Modify, Panel, Style, Width, object::Columns, themes::BorderCorrection};
use utils::get_nvme_pcie_info;

const TABLE_WIDTH: usize = 90;
const BANNER_WIDTH: usize = TABLE_WIDTH + 2;
const TABLE_INNER_WIDTH: usize = TABLE_WIDTH - 2;

pub struct TableFormatter {
    builder: Builder,
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
        TableFormatter { builder }
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
            Some("INFORMATION") => L10N
                .status_information()
                .bright_blue()
                .to_string(),
            Some("NEUTRAL") => "-".to_string(),
            Some("UNAVAILABLE") => L10N.unavailable().to_string(),
            _ => L10N.status_ok().green().to_string(),
        };

        let colored_value = match status {
            Some("KRITISCH") => value.red().bold().to_string(),
            Some("WARNUNG") => value.yellow().to_string(),
            Some("NEUTRAL" | "UNAVAILABLE" | "INFORMATION") => value.to_string(),
            _ => value.green().to_string(),
        };

        let colored_change = if change == "-" || change == "±0" {
            change.to_string()
        } else {
            change.bright_blue().to_string()
        };

        self.builder
            .push_record([name, &colored_value, &colored_change, &status_text]);
    }

    pub fn print_table(self, device: &StorageDevice, json: &Value) {
        let header_content = print_subheader(device, json);
        let table = self
            .builder
            .build()
            .with(Panel::header(header_content))
            .with(BorderCorrection::span())
            .with(Style::rounded())
            .with(Modify::new(Columns::last()).with(Alignment::center()))
            .with(Width::increase(TABLE_WIDTH))
            .to_string();
        println!("{}", table);
    }
}

pub fn print_header(title: &str) {
    println!("{}", "═".repeat(BANNER_WIDTH).cyan());

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let mut title_str = format!("{:^width$}", title, width = BANNER_WIDTH);
    let start = BANNER_WIDTH - version.len();
    title_str.replace_range(start.., &version);

    print!("{}", title_str[..start].cyan().bold());
    println!("{}", &title_str[start..]);
}

fn print_subheader(device: &StorageDevice, json: &Value) -> String {
    let mut header_content = String::new();
    header_content.push_str(&device_header_line(device, json));
    header_content.push('\n');

    for (field, label) in [("serial_number", L10N.serial_number_label()), ("firmware_version", L10N.firmware())] {
        if let Some(value) = json[field].as_str() {
            header_content.push_str(&format!("{}: {}\n", label, value.trim()));
        }
    }
    if let Some(bytes) = json["user_capacity"]["bytes"]
        .as_u64()
        .or_else(|| json["nvme_total_capacity"].as_u64())
    {
        header_content.push_str(&format!("{}: {:.2} GB\n", L10N.capacity(), bytes as f64 / 1e9));
    }

    if json["device"]["protocol"]
        .as_str()
        .map_or_else(|| device.is_nvme(), |protocol| protocol == "NVMe")
    {
        if let Some(model) = json["model_name"].as_str() {
            header_content.push_str(&format!("{}: {}\n", L10N.model_number(), model.trim()));
        }
        if let Some(nvme_version) = json["nvme_version"]["string"].as_str() {
            header_content.push_str(&format!("{}: {}\n", L10N.nvme_version(), nvme_version.trim()));
        }
        if device.interface == "nvme"
            && let Ok((current, maximum)) = get_nvme_pcie_info(&device.short_device_name)
        {
            header_content.push_str(&format!("{} {} (max: {})", L10N.transmission_mode(), current, maximum));
        }
    } else {
        if let Some(model) = json["model_name"].as_str() {
            header_content.push_str(&format!("{}: {}\n", L10N.device_model(), model.trim()));
        }
        if let Some(sata_version) = json["sata_version"]["string"].as_str() {
            header_content.push_str(&format!("{}: {}\n", L10N.sata_version(), sata_version.trim()));
        }
    }

    header_content
}

fn device_header_line(device: &StorageDevice, json: &Value) -> String {
    let left_plain = format!("{} ({})", device.device_path, device.interface);
    let left_colored = format!("{} ({})", device.device_path.green(), device.interface.cyan());

    let passed = json["smart_status"]["passed"].as_bool();
    let status = match passed {
        Some(true) => L10N.status_ok(),
        Some(false) => L10N.status_critical(),
        None => L10N.unavailable(),
    };
    let status_plain = format!("{}: {}", L10N.smart_status(), status);
    let status_colored = match passed {
        Some(true) => status_plain.green().to_string(),
        Some(false) => status_plain.red().bold().to_string(),
        None => status_plain.clone(),
    };

    let occupied = left_plain.chars().count() + status_plain.chars().count();
    let padding = TABLE_INNER_WIDTH
        .saturating_sub(occupied)
        .max(1);

    format!("{}{}{}", left_colored, " ".repeat(padding), status_colored)
}
