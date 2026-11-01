use super::{convert_lba_to_tb, nvme_controller_name};

#[test]
fn convert_lba_to_tb_formats_units_in_decimal_tb() {
    // NVMe units contain 1000 × 512 bytes; SATA LBAs contain 512 bytes.
    for (units, multiplier, expected) in [
        (1_953_125, 512000.0, "1.00 TB"),
        (1_953_125_000, 512.0, "1.00 TB"),
        (0, 512000.0, "0.00 TB"),
        (1_000_000, 512.0, "0.00 TB"),
        (7_812_500, 512000.0, "4.00 TB"),
    ] {
        assert_eq!(convert_lba_to_tb(units, multiplier), expected, "{units} units of {multiplier} bytes");
    }
}

#[test]
fn nvme_controller_name_strips_namespace() {
    assert_eq!(nvme_controller_name("nvme0n1"), "nvme0");
    assert_eq!(nvme_controller_name("nvme10n2"), "nvme10");
}

#[test]
fn nvme_controller_name_keeps_controller_and_unknown_names() {
    assert_eq!(nvme_controller_name("nvme0"), "nvme0");
    assert_eq!(nvme_controller_name("sda"), "sda");
}
