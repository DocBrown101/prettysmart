use crate::localization::L10N;
use crate::utils::{self, StorageDevice};
use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;
use tabled::settings::{Alignment, Modify, Panel, Style, Width, object::Columns, themes::BorderCorrection};
use utils::get_nvme_pcie_info;

pub fn print_header(title: &str) {
    let top_line = "═".repeat(70);
    println!("{}", top_line.cyan());
    println!("{:^70}", title.cyan().bold());
}

pub fn create_table_builder() -> Builder {
    let mut builder = Builder::default();
    builder.push_record([
        format!("{}", L10N.table_property().cyan()),
        format!("{}", L10N.table_value().cyan()),
        format!("{}", L10N.table_status().cyan()),
    ]);
    builder
}

pub fn add_row(builder: &mut Builder, name: &str, value: &str, status: Option<&str>) {
    let status_text = match status {
        Some("KRITISCH") => L10N.status_critical().red().to_string(),
        Some("WARNUNG") => L10N.status_warning().yellow().to_string(),
        None => L10N.status_ok().green().to_string(),
        Some(s) => s.to_string(),
    };

    let colored_value = match status {
        Some("KRITISCH") => value.red().bold().to_string(),
        Some("WARNUNG") => value.yellow().to_string(),
        _ => value.green().to_string(),
    };

    builder.push_record([name, &colored_value, &status_text]);
}

pub fn print_table(device: &StorageDevice, json: &Value, builder: Builder) {
    let header_content = print_subheader(&device, &json);
    let table = builder
        .build()
        .with(Panel::header(header_content))
        .with(BorderCorrection::span())
        .with(Style::rounded())
        .with(Modify::new(Columns::last()).with(Alignment::center()))
        .with(Width::increase(70)) // MinWidth 70
        .to_string();
    print!("{}", table);
}

fn print_subheader(device: &StorageDevice, json: &Value) -> String {
    let mut header_content = String::new();
    let device_colored = format!("✓ {}", device.device_path).green();
    header_content.push_str(&format!("{} ({})\n", device_colored, device.interface.cyan()));

    if device.interface == "nvme" {
        if let Some(model) = json["model_name"].as_str() {
            header_content.push_str(&format!("Model Number: {}\n", model.trim()));
        }
        if let Some(nvme_version) = json["nvme_version"]["string"].as_str() {
            header_content.push_str(&format!("NVMe Version: {}\n", nvme_version.trim()));
        }
        if let Ok((current, maximum)) = get_nvme_pcie_info(&device.short_device_name) {
            header_content.push_str(&format!("{} {} (max: {})", L10N.transmission_mode(), current, maximum));
        }
    } else {
        if let Some(model) = json["model_name"].as_str() {
            header_content.push_str(&format!("Device Model: {}\n", model.trim()));
        }
        if let Some(sata_version) = json["sata_version"]["string"].as_str() {
            header_content.push_str(&format!("SATA Version: {}\n", sata_version.trim()));
        }
    }

    header_content
}
