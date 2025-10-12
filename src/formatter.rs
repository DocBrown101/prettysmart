use crate::local_strings::LocalStrings;
use colored::Colorize;
use std::process::Output;
use tabled::builder::Builder;
use tabled::settings::{Alignment, Modify, Panel, Style, object::Columns};

pub fn print_header(title: &str) {
    let top_line = "═".repeat(70);
    println!("{}", top_line.cyan());
    println!("{:^70}", title.cyan().bold());
}

pub fn create_table_builder(strings: &LocalStrings) -> Builder {
    let mut builder = Builder::default();
    builder.push_record([
        format!("{}", strings.table_property().cyan()),
        format!("{}", strings.table_value().cyan()),
        format!("{}", strings.table_status().cyan()),
    ]);
    builder
}

pub fn add_row(builder: &mut Builder, strings: &LocalStrings, name: &str, value: &str, status: Option<&str>) {
    let status_text = match status {
        Some("KRITISCH") => strings.status_critical().red().to_string(),
        Some("WARNUNG") => strings.status_warning().yellow().to_string(),
        None => strings.status_ok().green().to_string(),
        Some(s) => s.to_string(),
    };

    let colored_value = match status {
        Some("KRITISCH") => value.red().bold().to_string(),
        Some("WARNUNG") => value.yellow().to_string(),
        _ => value.green().to_string(),
    };

    builder.push_record([name, &colored_value, &status_text]);
}

pub fn print_table(device: &str, device_type: &str, output: &Output, builder: Builder) {
    let header_content = print_subheader(&device, device_type, &output);
    let table = builder
        .build()
        .with(Panel::header(header_content))
        .with(Style::rounded())
        .with(Modify::new(Columns::last()).with(Alignment::center()))
        .to_string();
    println!("{}", table);
}

fn print_subheader(device: &str, device_type: &str, output: &Output) -> String {
    let mut header_content = String::new();
    let device_colored = format!("✓ {}", device).green();
    header_content.push_str(&format!("{} ({})\n", device_colored, device_type.cyan()));

    let info_str = String::from_utf8_lossy(&output.stdout);
    let keywords = if device_type == "NVMe" {
        vec!["Model Number", "NVMe Version", "Formatted LBA Size"]
    } else {
        vec!["Device Model", "Sector Size", "SATA Version"]
    };

    for line in info_str.lines() {
        if keywords.iter().any(|k| line.contains(k)) {
            header_content.push_str(&format!("{}\n", line.trim()));
        }
    }
    header_content
}
