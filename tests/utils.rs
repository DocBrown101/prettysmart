use prettysmart::utils::convert_lba_to_tb;

#[test]
fn convert_lba_to_tb_nvme_data_units() {
    // NVMe data units use 512000 byte multiplier (1000 × 512-byte blocks)
    // 1 TB = 1e12 bytes → 1e12 / 512000 ≈ 1_953_125 units
    assert_eq!(convert_lba_to_tb(1_953_125, 512000.0), "1.00 TB");
}

#[test]
fn convert_lba_to_tb_sata_lbas() {
    // SATA LBAs use 512 byte sectors
    // 1 TB = 1e12 bytes → 1e12 / 512 ≈ 1_953_125_000 LBAs
    assert_eq!(convert_lba_to_tb(1_953_125_000, 512.0), "1.00 TB");
}

#[test]
fn convert_lba_to_tb_zero() {
    assert_eq!(convert_lba_to_tb(0, 512000.0), "0.00 TB");
}

#[test]
fn convert_lba_to_tb_small_value() {
    // 1_000_000 units × 512 bytes = 512_000_000 bytes = 0.000512 TB
    assert_eq!(convert_lba_to_tb(1_000_000, 512.0), "0.00 TB");
}

#[test]
fn convert_lba_to_tb_large_value() {
    // ~4 TB worth of NVMe data units
    assert_eq!(convert_lba_to_tb(7_812_500, 512000.0), "4.00 TB");
}
