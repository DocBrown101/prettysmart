use prettysmart::formatter::TableFormatter;
use prettysmart::snapshot::{DeltaFormat, format_change};
use std::collections::BTreeMap;

pub(crate) struct MetricContext<'a> {
    pub(crate) formatter: &'a mut TableFormatter,
    previous_metrics: Option<&'a BTreeMap<String, u64>>,
    current_metrics: &'a mut BTreeMap<String, u64>,
}

impl<'a> MetricContext<'a> {
    pub(crate) fn new(
        formatter: &'a mut TableFormatter,
        previous_metrics: Option<&'a BTreeMap<String, u64>>,
        current_metrics: &'a mut BTreeMap<String, u64>,
    ) -> Self {
        Self {
            formatter,
            previous_metrics,
            current_metrics,
        }
    }

    pub(crate) fn previous_value(&self, key: &str) -> Option<u64> {
        self.previous_metrics
            .and_then(|metrics| metrics.get(key).copied())
    }

    pub(crate) fn add_row(&mut self, key: &str, name: &str, value: &str, current: u64, delta_format: DeltaFormat, status: Option<&str>) {
        let previous = self.previous_value(key);
        let change = format_change(previous, current, delta_format);
        // Percentage warnings describe current health, not new counter events.
        let status = if status == Some("WARNUNG") && !matches!(delta_format, DeltaFormat::Percent) && previous.is_none_or(|previous| previous == current) {
            None
        } else {
            status
        };

        if current > 0 || matches!(status, Some("KRITISCH" | "WARNUNG")) {
            self.formatter
                .add_row(name, value, &change, status);
        }
        self.current_metrics
            .insert(key.to_string(), current);
    }
}
