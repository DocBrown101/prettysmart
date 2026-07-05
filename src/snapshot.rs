use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SnapshotStore {
    #[serde(default)]
    devices: BTreeMap<String, Vec<DeviceSnapshot>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DeviceSnapshot {
    timestamp: u64,
    metrics: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug)]
pub enum DeltaFormat {
    Count,
    Hours,
    Minutes,
    Percent,
    Tb { multiplier: f64 },
    MonotonicCount,
    MonotonicHours,
    MonotonicMinutes,
    MonotonicTb { multiplier: f64 },
}

impl SnapshotStore {
    pub fn load() -> io::Result<Self> {
        let path = snapshot_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(Self::default());
        }

        serde_json::from_str(&content).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    pub fn save(&self) -> io::Result<()> {
        let path = snapshot_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let temporary_path = path.with_extension(format!("json.tmp.{}", std::process::id()));

        let mut temporary_file = File::create(&temporary_path)?;
        temporary_file.write_all(content.as_bytes())?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        if let Err(err) = fs::rename(&temporary_path, &path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(err);
        }

        Ok(())
    }

    pub fn latest_metrics(&self, serial_number: &str) -> Option<&BTreeMap<String, u64>> {
        self.devices
            .get(serial_number)
            .and_then(|snapshots| snapshots.last())
            .map(|snapshot| &snapshot.metrics)
    }

    pub fn append(&mut self, serial_number: &str, metrics: BTreeMap<String, u64>) {
        if metrics.is_empty() {
            return;
        }

        self.devices
            .entry(serial_number.to_string())
            .or_default()
            .push(DeviceSnapshot {
                timestamp: unix_timestamp(),
                metrics,
            });
    }
}

pub fn serial_number(json: &Value) -> Option<&str> {
    json["serial_number"]
        .as_str()
        .map(str::trim)
        .filter(|serial| !serial.is_empty())
}

pub fn format_change(previous: Option<u64>, current: u64, format: DeltaFormat) -> String {
    let Some(previous) = previous else {
        return "-".to_string();
    };

    let diff = current as i128 - previous as i128;
    if diff == 0 {
        return "±0".to_string();
    }

    if diff < 0 && format.is_monotonic() {
        return "reset?".to_string();
    }

    let sign = if diff > 0 { "+" } else { "-" };
    let absolute = diff.unsigned_abs();

    match format {
        DeltaFormat::Count | DeltaFormat::MonotonicCount => format!("{}{}", sign, absolute),
        DeltaFormat::Hours | DeltaFormat::MonotonicHours => format!("{}{} h", sign, absolute),
        DeltaFormat::Minutes | DeltaFormat::MonotonicMinutes => format!("{}{} min", sign, absolute),
        DeltaFormat::Percent => format!("{}{}%", sign, absolute),
        DeltaFormat::Tb { multiplier } | DeltaFormat::MonotonicTb { multiplier } => format_byte_change(sign, absolute, multiplier),
    }
}

fn format_byte_change(sign: &str, units: u128, multiplier: f64) -> String {
    let bytes = units as f64 * multiplier;

    if bytes < 1_000.0 {
        format!("{}{:.0} B", sign, bytes)
    } else if bytes < 1_000_000.0 {
        format!("{}{:.2} KB", sign, bytes / 1_000.0)
    } else if bytes < 1_000_000_000.0 {
        format!("{}{:.2} MB", sign, bytes / 1_000_000.0)
    } else if bytes < 1_000_000_000_000.0 {
        format!("{}{:.2} GB", sign, bytes / 1_000_000_000.0)
    } else {
        format!("{}{:.2} TB", sign, bytes / 1_000_000_000_000.0)
    }
}

impl DeltaFormat {
    fn is_monotonic(self) -> bool {
        matches!(
            self,
            DeltaFormat::MonotonicCount | DeltaFormat::MonotonicHours | DeltaFormat::MonotonicMinutes | DeltaFormat::MonotonicTb { .. }
        )
    }
}

fn snapshot_path() -> io::Result<PathBuf> {
    if let Ok(state_home) = env::var("XDG_STATE_HOME")
        && !state_home.trim().is_empty()
    {
        return Ok(PathBuf::from(state_home)
            .join("prettysmart")
            .join("snapshots.json"));
    }

    let home = env::var("HOME").map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("prettysmart")
        .join("snapshots.json"))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
