use prettysmart::localization::{Language, Localization};

#[test]
fn operating_hours_shows_correct_days() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.operating_hours(48), "48 h (2 days)");
}

#[test]
fn operating_hours_zero() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.operating_hours(0), "0 h (0 days)");
}

#[test]
fn operating_hours_less_than_a_day() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.operating_hours(23), "23 h (0 days)");
}

#[test]
fn operating_hours_large_value() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.operating_hours(8760), "8760 h (365 days)");
}

#[test]
fn smart_data_error() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.smart_data_error("/dev/sda"), "✗ /dev/sda - SMART data could not be retrieved");
}

#[test]
fn critical_warning() {
    let loc = Localization::new(Language::EN);
    assert_eq!(loc.critical_warning(3), "⚠️ CRITICAL WARNING: 3");
}

#[test]
fn localized_strings_differ_by_language() {
    let en = Localization::new(Language::EN);
    let de = Localization::new(Language::DE);
    assert_ne!(en.header_title(), de.header_title());
    assert_ne!(en.no_devices(), de.no_devices());
    assert_ne!(en.drive_health(), de.drive_health());
}
