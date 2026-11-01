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
        for variable in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = env::var(variable)
                && !value.trim().is_empty()
            {
                return if value.starts_with("de") { Language::DE } else { Language::EN };
            }
        }
        Language::EN
    }

    localized! {
        critical_comp_time => "Zeit über krit. Temperatur", "Critical comp time";
        critical_warning_label => "⚠️ KRITISCHE WARNUNG", "⚠️ CRITICAL WARNING";
        data_read_label => "Daten gelesen", "Data read";
        data_written_label => "Daten geschrieben", "Data written";
        nand_written_tlc => "NAND geschrieben (TLC)", "NAND written (TLC)";
        nand_written_slc => "NAND geschrieben (SLC)", "NAND written (SLC)";
        device_model => "Gerätemodell", "Device Model";
        drive_health => "Geschätzte Rest-Schreibausdauer", "Estimated remaining write endurance";
        header_title => "Speichermedien-Diagnose", "Storage Media Diagnostics";
        json_parse_error => "JSON-Parsing fehlgeschlagen", "JSON parsing failed";
        media_errors => "Medienfehler", "Media Errors";
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
        sata_version => "SATA-Version", "SATA Version";
        smartctl_start_error => "smartctl konnte nicht gestartet werden", "smartctl could not be started";
        spare_blocks => "Ersatzblöcke", "Spare blocks";
        threshold => "Schwelle", "threshold";
        spin_retry_count => "Spin Retry Count", "Spin Retry Count";
        status_critical => "❌ KRITISCH", "❌ CRITICAL";
        status_information => "Info", "Information";
        status_ok => "✓ OK", "✓ OK";
        status_warning => "⚠️ WARNUNG", "⚠️ WARNING";
        table_property => "Eigenschaft", "Property";
        table_change => "Änderung", "Change";
        table_status => "Status", "Status";
        table_value => "Aktueller Wert", "Current Value";
        manufacturer_values => "Weitere Herstellerwerte · ohne eigene Bewertung", "Other vendor values · without own assessment";
        thermal_throttling => "Thermische Drosselungen", "Thermal throttling";
        transmission_mode => "Übertragungsmodus:", "Transmission mode:";
        udma_crc_errors => "UDMA-CRC-Fehler", "UDMA CRC-err";
        unsafe_shutdowns => "Unsichere Abschaltungen", "Unsafe Shutdowns";
        endurance_used => "Geschätzter Ausdauerverbrauch", "Estimated endurance used";
        nvme_warning_persistent_memory => "Persistenter Speicher unzuverlässig/schreibgeschützt", "Persistent memory unreliable/read-only";
        unknown_warning_bits => "Unbekannte Warnbits", "Unknown warning bits";
        unavailable => "Nicht verfügbar", "Unavailable";
        unsupported_protocol => "Nicht unterstütztes Geräteprotokoll", "Unsupported device protocol";
        smart_data => "SMART-Daten", "SMART data";
        smart_status => "SMART-Gesamtstatus", "SMART overall status";
        serial_number_label => "Seriennummer", "Serial number";
        firmware => "Firmware", "Firmware";
        capacity => "Kapazität", "Capacity";
        failed_now => "Grenzwert aktuell verletzt", "Threshold failed now";
        reported_uncorrectable => "Gemeldete unkorrigierbare Fehler", "Reported uncorrectable errors";
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

#[cfg(test)]
#[path = "tests/localization.rs"]
mod tests;
