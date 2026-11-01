use super::{Language, Localization};

#[test]
fn value_column_keeps_its_original_label_in_both_languages() {
    assert_eq!(Localization::new(Language::DE).table_value(), "Aktueller Wert");
    assert_eq!(Localization::new(Language::EN).table_value(), "Current Value");
}

#[test]
fn operating_hours_formats_hours_and_whole_days() {
    for (language, hours, expected) in [
        (Language::EN, 0, "0 h (0 days)"),
        (Language::EN, 23, "23 h (0 days)"),
        (Language::EN, 48, "48 h (2 days)"),
        (Language::EN, 8760, "8760 h (365 days)"),
        (Language::DE, 48, "48 h (2 Tage)"),
    ] {
        assert_eq!(Localization::new(language).operating_hours(hours), expected);
    }
}

#[test]
fn smart_data_error_includes_device_path() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.smart_data_error("/dev/sda"), "✗ /dev/sda - SMART data could not be retrieved");
}
