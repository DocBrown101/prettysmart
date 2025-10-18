use std::env;

#[derive(Clone, Copy)]
pub enum Language {
    DE,
    EN,
}

pub struct LocalStrings {
    lang: Language,
}

impl LocalStrings {
    pub fn new() -> Self {
        let lang = Self::detect_language();
        LocalStrings { lang }
    }

    fn detect_language() -> Language {
        if let Ok(lang_var) = env::var("LANG") {
            if lang_var.starts_with("de") {
                return Language::DE;
            }
        }
        if let Ok(lang_var) = env::var("LC_ALL") {
            if lang_var.starts_with("de") {
                return Language::DE;
            }
        }
        Language::EN
    }

    pub fn no_devices(&self) -> &'static str {
        match self.lang {
            Language::DE => "Keine Laufwerke gefunden",
            Language::EN => "No drives found",
        }
    }

    pub fn header_title(&self) -> &'static str {
        match self.lang {
            Language::DE => "Speichermedien-Diagnose",
            Language::EN => "Storage Media Diagnostics",
        }
    }

    pub fn smartctl_start_error(&self) -> &'static str {
        match self.lang {
            Language::DE => "smartctl konnte nicht gestartet werden",
            Language::EN => "smartctl could not be started",
        }
    }

    pub fn smart_data_error(&self, device: &str) -> String {
        match self.lang {
            Language::DE => format!("✗ {} - SMART-Daten konnten nicht abgerufen werden", device),
            Language::EN => format!("✗ {} - SMART data could not be retrieved", device),
        }
    }

    pub fn critical_warning(&self, value: i64) -> String {
        match self.lang {
            Language::DE => format!("⚠️ KRITISCHE WARNUNG: {}", value),
            Language::EN => format!("⚠️ CRITICAL WARNING: {}", value),
        }
    }

    pub fn spare_blocks(&self) -> &'static str {
        match self.lang {
            Language::DE => "Verfügbare Ersatzblöcke",
            Language::EN => "Available Spare Blocks",
        }
    }

    pub fn table_property(&self) -> &'static str {
        match self.lang {
            Language::DE => "Eigenschaft",
            Language::EN => "Property",
        }
    }

    pub fn table_value(&self) -> &'static str {
        match self.lang {
            Language::DE => "Aktueller Wert",
            Language::EN => "Current Value",
        }
    }

    pub fn table_status(&self) -> &'static str {
        match self.lang {
            Language::DE => "Status",
            Language::EN => "Status",
        }
    }

    pub fn status_ok(&self) -> &'static str {
        match self.lang {
            Language::DE => "✓ OK",
            Language::EN => "✓ OK",
        }
    }

    pub fn status_warning(&self) -> &'static str {
        match self.lang {
            Language::DE => "⚠️ WARNUNG",
            Language::EN => "⚠️ WARNING",
        }
    }

    pub fn status_critical(&self) -> &'static str {
        match self.lang {
            Language::DE => "❌ KRITISCH",
            Language::EN => "❌ CRITICAL",
        }
    }

    pub fn drive_health(&self) -> &'static str {
        match self.lang {
            Language::DE => "Laufwerk-Gesundheit",
            Language::EN => "Drive Health",
        }
    }

    pub fn remaining(&self) -> &'static str {
        match self.lang {
            Language::DE => "% verbleibend",
            Language::EN => "% remaining",
        }
    }

    pub fn transmission_mode(&self) -> &'static str {
        match self.lang {
            Language::DE => "Übertragungsmodus:",
            Language::EN => "Transmission mode:",
        }
    }

    pub fn data_read_label(&self) -> &'static str {
        match self.lang {
            Language::DE => "Daten gelesen",
            Language::EN => "Data read",
        }
    }

    pub fn data_written_label(&self) -> &'static str {
        match self.lang {
            Language::DE => "Daten geschrieben",
            Language::EN => "Data written",
        }
    }

    pub fn operating_hours_label(&self) -> &'static str {
        match self.lang {
            Language::DE => "Betriebsstunden",
            Language::EN => "Operating hours",
        }
    }

    pub fn power_cycles_label(&self) -> &'static str {
        match self.lang {
            Language::DE => "Einschaltzyklen",
            Language::EN => "Power cycles",
        }
    }

    pub fn data_written_approx_label(&self) -> &'static str {
        match self.lang {
            Language::DE => "Daten geschrieben (ca.)",
            Language::EN => "Data written (approx.)",
        }
    }

    pub fn unsafe_shutdowns(&self) -> &'static str {
        match self.lang {
            Language::DE => "Unsichere Abschaltungen",
            Language::EN => "Unsafe Shutdowns",
        }
    }

    pub fn media_errors(&self) -> &'static str {
        match self.lang {
            Language::DE => "Medienfehler",
            Language::EN => "Media Errors",
        }
    }

    pub fn reallocated_sectors(&self) -> &'static str {
        match self.lang {
            Language::DE => "Reallocated Sectors",
            Language::EN => "Reallocated Sectors",
        }
    }

    pub fn spin_retry_count(&self) -> &'static str {
        match self.lang {
            Language::DE => "Spin Retry Count",
            Language::EN => "Spin Retry Count",
        }
    }

    pub fn drive_health_remaining(&self) -> &'static str {
        match self.lang {
            Language::DE => "Drive Health (verbleibend)",
            Language::EN => "Drive Health (remaining)",
        }
    }

    pub fn json_parse_error(&self) -> &'static str {
        match self.lang {
            Language::DE => "JSON-Parsing fehlgeschlagen",
            Language::EN => "JSON parsing failed",
        }
    }
}
