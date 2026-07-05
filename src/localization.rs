use std::env;
use std::sync::LazyLock;

#[derive(Clone, Copy)]
pub enum Language {
    DE,
    EN,
}

pub static L10N: LazyLock<Localization> = LazyLock::new(|| Localization::new(Localization::detect_language()));

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
    pub fn new(lang: Language) -> Self {
        Self { lang }
    }

    fn detect_language() -> Language {
        if let Ok(lang_var) = env::var("LANG")
            && lang_var.starts_with("de")
        {
            return Language::DE;
        }
        if let Ok(lang_var) = env::var("LC_ALL")
            && lang_var.starts_with("de")
        {
            return Language::DE;
        }
        Language::EN
    }

    localized! {
        critical_comp_time => "Zeit über krit. Temperatur", "Critical comp time";
        critical_warning_label => "⚠️ KRITISCHE WARNUNG", "⚠️ CRITICAL WARNING";
        data_read_label => "Daten gelesen", "Data read";
        data_read_total => "Gelesene Daten (gesamt)", "Total data read";
        data_written_approx_label => "Daten geschrieben (ca.)", "Data written (approx.)";
        data_written_label => "Daten geschrieben", "Data written";
        device_model => "Gerätemodell", "Device Model";
        drive_health => "Laufwerk-Gesundheit", "Drive Health";
        drive_health_remaining => "Drive Health (verbleibend)", "Drive Health (remaining)";
        header_title => "Speichermedien-Diagnose", "Storage Media Diagnostics";
        json_parse_error => "JSON-Parsing fehlgeschlagen", "JSON parsing failed";
        media_errors => "Medienfehler", "Media Errors";
        media_wearout_indicator => "SSD-Verschleiß", "Media Wearout Indicator";
        model_number => "Modellnummer", "Model Number";
        no_devices => "Keine Laufwerke gefunden", "No drives found";
        num_err_log_entries => "Fehlerprotokoll-Einträge", "Num err log entries";
        nvme_version => "NVMe-Version", "NVMe Version";
        nvme_warning_read_only => "Medium schreibgeschützt", "Media read-only";
        nvme_warning_reliability => "Zuverlässigkeit degradiert", "Reliability degraded";
        nvme_warning_spare_capacity => "Spare-Kapazität unter Schwellwert", "Spare capacity below threshold";
        nvme_warning_temperature => "Temperatur über Schwellwert", "Temperature above threshold";
        nvme_warning_volatile_backup => "Volatile Memory Backup fehlgeschlagen", "Volatile memory backup failed";
        offline_uncorrectable_sectors => "Nicht korrigierbare Sektoren", "Offline uncorrectable sectors";
        operating_hours_label => "Betriebsstunden", "Operating hours";
        operating_hours_value => "{} h ({} Tage)", "{} h ({} days)";
        overall_throttled => "Insgesamt gedrosselt", "Overall throttled";
        pending_sectors => "Ausstehende Sektoren", "Pending sectors";
        power_cycles_label => "Einschaltzyklen", "Power cycles";
        reallocated_sectors => "Reallocated Sectors", "Reallocated Sectors";
        remaining => "% verbleibend", "% remaining";
        reserved_capacity_available => "Reservierte Kapazität verfügbar", "Reserved Capacity Available";
        reserved_space_alt => "Res. Speicher (Alt)", "Reserved Space (Alt)";
        sata_version => "SATA-Version", "SATA Version";
        smartctl_start_error => "smartctl konnte nicht gestartet werden", "smartctl could not be started";
        spare_blocks => "Verfügbare Ersatzblöcke", "Available Spare Blocks";
        spin_retry_count => "Spin Retry Count", "Spin Retry Count";
        ssd_life_remaining => "SSD-Lebensdauer verbleibend", "SSD Life Remaining";
        status_critical => "❌ KRITISCH", "❌ CRITICAL";
        status_information => "Info", "Information";
        status_ok => "✓ OK", "✓ OK";
        status_warning => "⚠️ WARNUNG", "⚠️ WARNING";
        table_property => "Eigenschaft", "Property";
        table_change => "Veränderung", "Change";
        table_status => "Status", "Status";
        table_value => "Aktueller Wert", "Current Value";
        thermal_throttling => "Thermische Drosselungen", "Thermal throttling";
        transmission_mode => "Übertragungsmodus:", "Transmission mode:";
        udma_crc_errors => "UDMA-CRC-Fehler", "UDMA CRC-err";
        unsafe_shutdowns => "Unsichere Abschaltungen", "Unsafe Shutdowns";
        wear_leveling => "Verschleißausgleich", "Wear Leveling";
    }

    pub fn smart_data_error(&self, device: &str) -> String {
        match self.lang {
            Language::DE => format!("✗ {} - SMART-Daten konnten nicht abgerufen werden", device),
            Language::EN => format!("✗ {} - SMART data could not be retrieved", device),
        }
    }

    pub fn critical_warning(&self, value: u64) -> String {
        format!("{}: {}", self.critical_warning_label(), value)
    }

    pub fn operating_hours(&self, hours: u64) -> String {
        self.operating_hours_value()
            .replacen("{}", &hours.to_string(), 1)
            .replacen("{}", &(hours / 24).to_string(), 1)
    }

    pub fn snapshot_load_error(&self, error: &str) -> String {
        match self.lang {
            Language::DE => format!("Snapshot-Historie konnte nicht geladen werden: {}", error),
            Language::EN => format!("Snapshot history could not be loaded: {}", error),
        }
    }

    pub fn snapshot_save_error(&self, error: &str) -> String {
        match self.lang {
            Language::DE => format!("Snapshot-Historie konnte nicht gespeichert werden: {}", error),
            Language::EN => format!("Snapshot history could not be saved: {}", error),
        }
    }
}
