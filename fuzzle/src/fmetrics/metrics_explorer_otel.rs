// https://github.com/palindrom615/metrics-exporter-otel/blob/otel-publish/metrics-exporter-otel/src

// mod instruments;
// mod metadata;
// mod storage;
// /// FIXME this module is for temporary patch of [metrics]
// mod metrics_ext;

use metrics::{Counter, Gauge, Histogram, Key, KeyName, Recorder, SharedString, Unit};
use metrics_util::registry::Registry;
use metrics_util::MetricKind;
use opentelemetry::metrics::Meter;
use std::sync::Arc;

/// A [`Recorder`] that exports metrics to OpenTelemetry.
///
/// Clone is shallow; Clones share the same underlying data.
///
/// ```rust,no_run
/// use opentelemetry::metrics::MeterProvider;
/// use metrics_exporter_otel::OpenTelemetryRecorder;
/// use opentelemetry_sdk::metrics::SdkMeterProvider;
///
/// let provider = SdkMeterProvider::default();
/// let meter = provider.meter("my_app");
/// let recorder = OpenTelemetryRecorder::new(meter);
///
/// metrics::set_global_recorder(recorder).expect("failed to install recorder");
/// ```
#[derive(Clone)]
pub struct OpenTelemetryRecorder {
    registry: Arc<Registry<Key, OtelMetricStorage>>,
    metadata: MetricMetadata,
}

impl OpenTelemetryRecorder {
    /// Creates a new OpenTelemetry recorder with the given meter.
    pub fn new(meter: Meter) -> Self {
        let metadata = MetricMetadata::new();
        let storage = OtelMetricStorage::new(meter, metadata.clone());
        Self { registry: Arc::new(Registry::new(storage)), metadata }
    }

    /// Sets custom bucket boundaries for a histogram metric.
    ///
    /// Must be called before the histogram is first created. Boundaries cannot be
    /// changed after a histogram has been created.
    pub fn set_histogram_bounds(&self, key: &KeyName, bounds: Vec<f64>) {
        self.metadata.set_histogram_bounds(key.clone(), bounds);
    }

    /// Gets a description entry for testing purposes.
    #[cfg(test)]
    pub fn get_description(
        &self,
        key_name: KeyName,
        metric_kind: MetricKind,
    ) -> Option<MetricDescription> {
        self.metadata.get_description(&key_name, metric_kind)
    }
}

impl Recorder for OpenTelemetryRecorder {
    fn describe_counter(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.metadata.set_description(key_name, MetricKind::Counter, unit, description);
    }

    fn describe_gauge(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.metadata.set_description(key_name, MetricKind::Gauge, unit, description);
    }

    fn describe_histogram(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.metadata.set_description(key_name, MetricKind::Histogram, unit, description);
    }

    fn register_counter(&self, key: &Key, _metadata: &metrics::Metadata<'_>) -> Counter {
        self.registry.get_or_create_counter(key, |c| Counter::from_arc(c.clone()))
    }

    fn register_gauge(&self, key: &Key, _metadata: &metrics::Metadata<'_>) -> Gauge {
        self.registry.get_or_create_gauge(key, |g| Gauge::from_arc(g.clone()))
    }

    fn register_histogram(&self, key: &Key, _metadata: &metrics::Metadata<'_>) -> Histogram {
        self.registry.get_or_create_histogram(key, |h| Histogram::from_arc(h.clone()))
    }
}

use scc::HashMap;

/// A metric description containing unit and textual description.
///
/// This structure holds the metadata associated with a metric, including its unit of
/// measurement and human-readable description. This information is used to enrich
/// the OpenTelemetry metric output.
#[derive(Clone)]
pub struct MetricDescription {
    /// The unit of measurement for this metric (e.g., bytes, seconds, count)
    unit: Option<Unit>,
    /// Human-readable description of what this metric measures
    description: SharedString,
}

impl MetricDescription {
    /// Returns the unit of measurement for this metric.
    pub fn unit(&self) -> Option<Unit> {
        self.unit
    }

    /// Returns the human-readable description of this metric.
    pub fn description(&self) -> SharedString {
        self.description.clone()
    }
}

/// Stores all metric metadata including descriptions and histogram bounds.
///
/// This structure maintains a centralized store of metadata for all metrics, providing
/// lock-free concurrent access through SCC (Scalable Concurrent Collections) HashMaps.
/// It stores both metric descriptions (with units) and custom histogram bucket boundaries.
///
/// # Thread Safety
///
/// This structure is designed for high-performance concurrent access. Multiple threads
/// can safely read and write metadata simultaneously with minimal contention.
#[derive(Clone, Default)]
pub struct MetricMetadata {
    descriptions: Arc<HashMap<(KeyName, MetricKind), MetricDescription>>,
    histogram_bounds: Arc<HashMap<KeyName, Vec<f64>>>,
}

impl MetricMetadata {
    pub fn new() -> Self {
        Self { descriptions: Arc::new(HashMap::new()), histogram_bounds: Arc::new(HashMap::new()) }
    }

    pub fn set_description(
        &self,
        key_name: KeyName,
        metric_kind: MetricKind,
        unit: Option<Unit>,
        description: SharedString,
    ) {
        let new_entry = MetricDescription { unit, description };
        let _ = self.descriptions.insert_sync((key_name, metric_kind), new_entry);
    }

    pub fn get_description(
        &self,
        key_name: &KeyName,
        metric_kind: MetricKind,
    ) -> Option<MetricDescription> {
        self.descriptions.read_sync(&(key_name.clone(), metric_kind), |_, v| v.clone())
    }

    pub fn set_histogram_bounds(&self, key_name: KeyName, bounds: Vec<f64>) {
        let _ = self.histogram_bounds.insert_sync(key_name, bounds);
    }

    pub fn get_histogram_bounds(&self, key_name: &KeyName) -> Option<Vec<f64>> {
        self.histogram_bounds.read_sync(key_name, |_, v| v.clone())
    }
}


pub(crate) trait UnitExt {
    fn as_ucum_label(&self) -> &'static str;
}

impl UnitExt for Unit {
    /// Gets the notation of Unified Code for Units of Measure (UCUM) for given unit
    ///
    /// This is useful for metric systems using UCUM, like [OpenTelemetry](https://opentelemetry.io/docs/specs/semconv/general/metrics/#instrument-units)
    ///
    /// See Also <https://ucum.org/>
    fn as_ucum_label(&self) -> &'static str {
        match self {
            // dimensionless
            Unit::Count            => "1",
            Unit::Percent          => "%",

            // time
            Unit::Seconds          => "s",
            Unit::Milliseconds     => "ms",
            Unit::Microseconds     => "us",
            Unit::Nanoseconds      => "ns",

            // storage (binary prefixes)
            Unit::Tebibytes        => "TiBy",
            Unit::Gibibytes        => "GiBy",
            Unit::Mebibytes        => "MiBy",
            Unit::Kibibytes        => "KiBy",
            Unit::Bytes            => "By",

            // network throughput
            Unit::TerabitsPerSecond => "Tbit/s",
            Unit::GigabitsPerSecond => "Gbit/s",
            Unit::MegabitsPerSecond => "Mbit/s",
            Unit::KilobitsPerSecond => "kbit/s",
            Unit::BitsPerSecond     => "bit/s",

            // event rate
            Unit::CountPerSecond    => "1/s",
        }
    }
}

use metrics_util::registry::Storage;
use opentelemetry::metrics::{AsyncInstrumentBuilder, HistogramBuilder};
use opentelemetry::KeyValue;

pub struct OtelMetricStorage {
    meter: Meter,
    metadata: MetricMetadata,
}

impl OtelMetricStorage {
    pub fn new(meter: Meter, metadata: MetricMetadata) -> Self {
        Self { meter, metadata }
    }

    fn get_attributes(key: &Key) -> Vec<KeyValue> {
        key.labels()
            .map(|label| KeyValue::new(label.key().to_string(), label.value().to_string()))
            .collect()
    }

    fn with_description<'a, I, M>(
        description: &MetricDescription,
        builder: AsyncInstrumentBuilder<'a, I, M>,
    ) -> AsyncInstrumentBuilder<'a, I, M> {
        match description.unit() {
            Some(unit) => builder
                .with_description(description.description().to_string())
                .with_unit(unit.as_ucum_label()),
            None => builder.with_description(description.description().to_string()),
        }
    }

    fn with_description_histogram<'a, T>(
        description: &MetricDescription,
        builder: HistogramBuilder<'a, T>,
    ) -> HistogramBuilder<'a, T> {
        match description.unit() {
            Some(unit) => builder
                .with_description(description.description().to_string())
                .with_unit(unit.as_ucum_label()),
            None => builder.with_description(description.description().to_string()),
        }
    }
}

impl Storage<Key> for OtelMetricStorage {
    type Counter = Arc<OtelCounter>;
    type Gauge = Arc<OtelGauge>;
    type Histogram = Arc<OtelHistogram>;

    fn counter(&self, key: &Key) -> Self::Counter {
        let builder = self.meter.u64_observable_counter(key.name().to_string());
        let key_name = KeyName::from(key.name().to_string());
        let builder = if let Some(description) =
            self.metadata.get_description(&key_name, MetricKind::Counter)
        {
            Self::with_description(&description, builder)
        } else {
            builder
        };
        let attributes = Self::get_attributes(key);
        Arc::new(OtelCounter::new(builder, attributes))
    }

    fn gauge(&self, key: &Key) -> Self::Gauge {
        let builder = self.meter.f64_observable_gauge(key.name().to_string());
        let key_name = KeyName::from(key.name().to_string());
        let builder = if let Some(description) =
            self.metadata.get_description(&key_name, MetricKind::Gauge)
        {
            Self::with_description(&description, builder)
        } else {
            builder
        };
        let attributes = Self::get_attributes(key);
        Arc::new(OtelGauge::new(builder, attributes))
    }

    fn histogram(&self, key: &Key) -> Self::Histogram {
        let builder = self.meter.f64_histogram(key.name().to_string());
        let key_name = KeyName::from(key.name().to_string());

        let builder = if let Some(description) =
            self.metadata.get_description(&key_name, MetricKind::Histogram)
        {
            Self::with_description_histogram(&description, builder)
        } else {
            builder
        };

        // Apply histogram bounds if they exist
        let builder = if let Some(bounds) = self.metadata.get_histogram_bounds(&key_name) {
            builder.with_boundaries(bounds)
        } else {
            builder
        };

        let attributes = Self::get_attributes(key);
        Arc::new(OtelHistogram::new(builder.build(), attributes))
    }
}

use metrics::{CounterFn, GaugeFn, HistogramFn};
use opentelemetry::metrics::{
    Histogram as OHistogram, ObservableCounter, ObservableGauge,
};
use portable_atomic::{AtomicF64, Ordering};
use std::sync::atomic::AtomicU64;

pub struct OtelCounter {
    #[allow(dead_code)] // prevent from drop
    counter: ObservableCounter<u64>,
    value: Arc<AtomicU64>,
}

impl OtelCounter {
    pub fn new(
        counter_builder: AsyncInstrumentBuilder<ObservableCounter<u64>, u64>,
        attributes: Vec<KeyValue>,
    ) -> Self {
        let value = Arc::new(AtomicU64::new(0));
        let value_moved = Arc::clone(&value);
        let otel_counter = counter_builder
            .with_callback(move |observer| {
                observer.observe(value_moved.load(Ordering::Relaxed), &attributes);
            })
            .build();
        Self { counter: otel_counter, value }
    }
}

impl CounterFn for OtelCounter {
    fn increment(&self, value: u64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    fn absolute(&self, value: u64) {
        self.value.store(value, Ordering::Relaxed);
    }
}

pub struct OtelGauge {
    #[allow(dead_code)] // prevent from drop
    gauge: ObservableGauge<f64>,
    value: Arc<AtomicF64>,
}

impl OtelGauge {
    pub fn new(
        gauge_builder: AsyncInstrumentBuilder<ObservableGauge<f64>, f64>,
        attributes: Vec<KeyValue>,
    ) -> Self {
        let value = Arc::new(AtomicF64::new(0.0));
        let value_moved = value.clone();
        let otel_gauge = gauge_builder
            .with_callback(move |observer| {
                observer.observe(value_moved.load(Ordering::Relaxed), &attributes);
            })
            .build();
        Self { gauge: otel_gauge, value }
    }
}

impl GaugeFn for OtelGauge {
    fn increment(&self, value: f64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    fn decrement(&self, value: f64) {
        self.value.fetch_sub(value, Ordering::Relaxed);
    }

    fn set(&self, value: f64) {
        self.value.store(value, Ordering::Relaxed);
    }
}

pub struct OtelHistogram {
    histogram: OHistogram<f64>,
    attributes: Vec<KeyValue>,
}

impl OtelHistogram {
    pub fn new(histogram: OHistogram<f64>, attributes: Vec<KeyValue>) -> Self {
        Self { histogram, attributes }
    }
}

impl HistogramFn for OtelHistogram {
    fn record(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }
}

