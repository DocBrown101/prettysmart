use std::env;
use std::sync::LazyLock;

#[derive(Clone, Copy)]
pub enum Language {
    DE,
    EN,
}

pub static L10N: LazyLock<Localization> = LazyLock::new(|| Localization::new());

pub struct Localization {
    lang: Language,
}

macro_rules! localized {
    ($($name:ident => $de:expr, $en:expr);* $(;)?) => {
        $(
            pub fn $name(&self) -> &'static str {
                match self.lang {
                    Language::DE => $de,
                    Language::EN => $en,
                }
            }
        )*
    };
}

impl Localization {
    pub fn new() -> Self {
        let lang = Self::detect_language();
        Localization { lang }
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

    localized! {
        critical_comp_time => "Zeit über krit. Temperatur", "Critical comp time";
        data_read_label => "Daten gelesen", "Data read";
        data_written_approx_label => "Daten geschrieben (ca.)", "Data written (approx.)";
        data_written_label => "Daten geschrieben", "Data written";
        drive_health => "Laufwerk-Gesundheit", "Drive Health";
        drive_health_remaining => "Drive Health (verbleibend)", "Drive Health (remaining)";
        header_title => "Speichermedien-Diagnose", "Storage Media Diagnostics";
        json_parse_error => "JSON-Parsing fehlgeschlagen", "JSON parsing failed";
        media_errors => "Medienfehler", "Media Errors";
        no_devices => "Keine Laufwerke gefunden", "No drives found";
        num_err_log_entries => "Fehlerprotokoll-Einträge", "Num err log entries";
        offline_uncorrectable_sectors => "Nicht korrigierbare Sektoren", "Offline uncorrectable sectors";
        operating_hours_label => "Betriebsstunden", "Operating hours";
        overall_throttled => "Insgesamt gedrosselt", "Overall throttled";
        pending_sectors => "Ausstehende Sektoren", "Pending sectors";
        power_cycles_label => "Einschaltzyklen", "Power cycles";
        reallocated_sectors => "Reallocated Sectors", "Reallocated Sectors";
        remaining => "% verbleibend", "% remaining";
        reserved_capacity_available => "Reservierte Kapazität verfügbar", "Reserved Capacity Available";
        smartctl_start_error => "smartctl konnte nicht gestartet werden", "smartctl could not be started";
        spare_blocks => "Verfügbare Ersatzblöcke", "Available Spare Blocks";
        spin_retry_count => "Spin Retry Count", "Spin Retry Count";
        ssd_life_remaining => "SSD-Lebensdauer verbleibend", "SSD Life Remaining";
        status_critical => "❌ KRITISCH", "❌ CRITICAL";
        status_ok => "✓ OK", "✓ OK";
        status_warning => "⚠️ WARNUNG", "⚠️ WARNING";
        status_information => "Info", "Information";
        table_property => "Eigenschaft", "Property";
        table_status => "Status", "Status";
        table_value => "Aktueller Wert", "Current Value";
        thermal_throttling => "Thermische Drosselungen", "Thermal throttling";
        transmission_mode => "Übertragungsmodus:", "Transmission mode:";
        unsafe_shutdowns => "Unsichere Abschaltungen", "Unsafe Shutdowns";
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
}
