mod metrics_explorer_otel;
mod system_metrics;

use std::{net::SocketAddr, str::FromStr, sync::Arc};

use anyhow::Context;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, trace::TraceContextExt};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::{
    logs::SdkLoggerProvider, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    resource::Resource, trace as sdktrace,
};
use pyroscope::pyroscope::PyroscopeAgentRunning;
use pyroscope::PyroscopeAgent;
use pyroscope_pprofrs::{pprof_backend, PprofConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Instrument, Level};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::fmetrics::system_metrics::start_system_metrics_task;

use self::{metrics_explorer_otel::OpenTelemetryRecorder};

use tracing::Span;

#[derive(Clone)]
pub struct TracedMessage<T> {
    pub message: T,
    pub span: Span,
}

pub trait TracedRequest {
    async fn send_traced(self, span_name: &str) -> reqwest::Result<reqwest::Response>;
}

impl TracedRequest for reqwest::RequestBuilder {
    async fn send_traced(mut self, span_name: &str) -> reqwest::Result<reqwest::Response> {
        let span = tracing::info_span!(
            "http_request",
            "otel.name" = span_name,
            "otel.kind" = "Client",
            "http.status_code" = tracing::field::Empty,
            "otel.status_code" = tracing::field::Empty
        );

        let cx = span.context();

        let mut headers = reqwest::header::HeaderMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(&mut headers));
        });

        for (k, v) in headers {
            if let Some(name) = k {
                self = self.header(name, v);
            }
        }

        async move {
            let result = self.send().await;

            match &result {
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    tracing::Span::current().record("http.status_code", code);

                    if !resp.status().is_success() {
                        tracing::Span::current().record("otel.status_code", "ERROR");
                        tracing::error!(status = code, "Request failed");
                    } else {
                        tracing::Span::current().record("otel.status_code", "OK");
                    }
                }
                Err(e) => {
                    tracing::Span::current().record("otel.status_code", "ERROR");
                    tracing::error!(error = %e, "Request error");
                }
            }

            result
        }
        .instrument(span)
        .await
    }
}

trait WithOtelContext {
    fn inject_context(self, cx: &opentelemetry::Context) -> Self;
}

impl WithOtelContext for reqwest::RequestBuilder {
    fn inject_context(self, cx: &opentelemetry::Context) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(cx, &mut HeaderInjector(&mut headers));
        });

        let mut builder = self;
        for (k, v) in headers {
            if let Some(name) = k {
                builder = builder.header(name, v);
            }
        }
        builder
    }
}

struct HeaderInjector<'a>(pub &'a mut reqwest::header::HeaderMap);

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = reqwest::header::HeaderName::from_str(key) {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&value) {
                self.0.insert(name, val);
            }
        }
    }
}

pub struct Observability {
    profile: PyroscopeAgent<PyroscopeAgentRunning>,
    trace: SdkTracerProvider,
    meter: SdkMeterProvider,
    logs: SdkLoggerProvider,
}

impl Drop for Observability {
    fn drop(&mut self) {
        // match self.profile.stop() {
        //     Ok(profile) => profile.shutdown(),
        //     Err(err) =>
        //     tracing::error!(error = %err, "Error shutting down profiling")
        // }
        if let Err(err) = self.meter.shutdown() {
            tracing::error!(error = %err, "Error shutting down meter");
        }
        if let Err(err) = self.logs.shutdown() {
            tracing::error!(error = %err, "Error shutting down logging");
        }
        if let Err(err) = self.trace.shutdown() {
            tracing::error!(error = %err, "Error shutting down tracing");
        }
    }
}

pub async fn setup_observability(
    otlp_endpoint: String,
    pyroscope_url: String,
) -> anyhow::Result<Observability> {
    let resource = Resource::builder().with_service_name("fuzzle-bot").build();

    let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint.clone())
        .build()
        .context("build OTLP trace exporter")?;

    let trace_provider = sdktrace::SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(resource.clone())
        .build();

    global::set_tracer_provider(trace_provider.clone());
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = trace_provider.tracer("rust-alloy-demo");

    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint.clone())
        .build()
        .context("build OTLP metric exporter")?;

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource.clone())
        .with_periodic_exporter(metric_exporter)
        .build();

    global::set_meter_provider(meter_provider.clone());

    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint.clone())
        .build()
        .context("build OTLP log exporter")?;

    let log_provider = SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(log_exporter)
        .build();

    let otel_log_layer =
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&log_provider);

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,opentelemetry=warn,Pyroscope=warn")),
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(otel_log_layer)
        .init();

    let meter = global::meter("rust-alloy-demo");
    let recorder = OpenTelemetryRecorder::new(meter);
    metrics::set_global_recorder(recorder).expect("failed to install recorder");
    metrics::describe_counter!(
        "requests_total",
        metrics::Unit::Count,
        "Total HTTP requests"
    );

    tokio::task::spawn(
        tokio_metrics::RuntimeMetricsReporterBuilder::default()
            .with_interval(std::time::Duration::from_secs(10))
            .describe_and_run(),
    );
    start_system_metrics_task();

    let pprof_config = PprofConfig::new()
        .sample_rate(100)
        .report_thread_id()
        .report_thread_name();
    let agent = PyroscopeAgent::builder(pyroscope_url.clone(), "fuzzle-bot".to_string())
        .backend(pprof_backend(pprof_config))
        .tags(vec![("env", "production")]) // Optional: Add default tags
        .build()?;

    let profile_agent = agent.start()?;

    info!(%otlp_endpoint,%pyroscope_url, "started observability stuff");

    Ok(Observability {
        logs: log_provider,
        meter: meter_provider,
        trace: trace_provider,
        profile: profile_agent,
    })
}
