# Project Overview

**prettysmart** is a command-line tool for monitoring and analyzing SMART (Self-Monitoring, Analysis, and Reporting Technology) data from storage devices. It provides detailed information about the health status of NVMe and SATA drives in an easy-to-read format.

<p align="center">
  <a href="https://sonarcloud.io/summary/new_code?id=DocBrown101_prettysmart">
    <img src="https://sonarcloud.io/api/project_badges/quality_gate?project=DocBrown101_prettysmart" />
  </a>
</p>

<p align="center">
  <img src="https://github.com/DocBrown101/prettysmart/blob/main/images/single_nvme.jpg" />
</p>

## Features

- **NVMe Support**: Comprehensive monitoring of NVMe SSDs including:
  - Drive health percentage
  - Available spare blocks
  - Temperature monitoring
  - Power-on hours and cycles
  - Data units read/written
  - Media errors and error log entries
  - Thermal throttling events
  - PCIe interface information (generation and lane width)

- **SATA Support**: Monitoring of SATA drives including:
  - Reallocated sectors
  - Pending sectors
  - Offline uncorrectable sectors
  - UDMA CRC errors
  - Spin retry count
  - Drive health remaining
  - SSD life remaining (for SSDs)
  - Reserved capacity available
  - Media wearout indicator
  - Wear leveling count
  - Total data read/written

- **Multi-language Support**: German and English interfaces
- **Color-coded Output**: Easy-to-read status indicators with color coding for critical, warning, and normal states
- **Tabular Format**: Well-organized table display of SMART attributes

## Installation

### Prerequisites

- Rust toolchain (rustc, cargo)
- `smartctl` from smartmontools package

### Building from Source

Clone the repository and build:
```bash
git clone https://github.com/DocBrown101/prettysmart.git
cd prettysmart
cargo build --release
```

The binary will be created at `target/release/prettysmart`.

## Usage

Simply run the tool (requires root privileges):
```bash
sudo ./target/release/prettysmart
```

Or install it to your PATH:
```bash
sudo cp target/release/prettysmart /usr/local/bin/
sudo prettysmart
```

## Language Selection

The tool automatically detects the system language based on `LANG` or `LC_ALL` environment variables. To force a specific language:

For German:
```bash
sudo LANG=de_DE.UTF-8 ./target/release/prettysmart
```

For English:
```bash
sudo LANG=en_US.UTF-8 ./target/release/prettysmart
```

## Output Example

The tool displays a table with SMART attributes for each detected storage device, including:
- Device model and interface type
- Current health status
- Warning indicators (if any)
- Detailed metrics in a tabular format

## Build Commands

### Format Code
```bash
cargo fmt
```

### Lint Code
```bash
cargo clippy --all-targets --all-features
```

### Build Release
```bash
cargo build --release
```

## Supported Devices

- NVMe SSDs (via `/dev/nvme*`)  
- SATA drives (via `/dev/sd*`, `/dev/hd*`)
