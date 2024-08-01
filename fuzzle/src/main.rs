use std::collections::HashMap;

use anyhow::Result;
use futures::Future;
use fuzzle_bot::{Config, UpdateListener};
use tokio::fs::{read_to_string, write, File};

use opentelemetry_sdk::trace::Tracer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Rust's default thread stack size of 2MiB doesn't allow sufficient recursion depth.
pub fn with_enough_stack<T>(fut: impl Future<Output = T> + Send) -> T {
    let stack_size = 10 * 1024 * 1024; // 10MiB

    // Stack frames are generally larger in debug mode.
    #[cfg(debug_assertions)]
    let stack_size = stack_size * 2;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(stack_size)
        .build()
        .expect("runtime to initialize")
        .block_on(fut)
}

fn create_otlp_tracer() -> Option<Tracer> {
    if !std::env::vars().any(|(name, _)| name.starts_with("OTEL_")) {
        return None;
    }
    let protocol = std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").unwrap_or("grpc".to_string());

    let tracer = opentelemetry_otlp::new_pipeline().tracing();

    let tracer = match protocol.as_str() {
        "grpc" => {
            let mut exporter = opentelemetry_otlp::new_exporter().tonic();

            // Check if we need TLS
            if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
                if endpoint.starts_with("https") {
                    exporter = exporter.with_tls_config(Default::default());
                }
            }
            tracer.with_exporter(exporter)
        }
        "http/protobuf" => {
            let exporter = opentelemetry_otlp::new_exporter().http();
            tracer.with_exporter(exporter)
        }
        p => panic!("Unsupported protocol {}", p),
    };

    Some(
        tracer
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("tracer install to be valid"),
    )
}

pub async fn init() -> Result<()> {
    let settings = config::Config::builder()
        // sticker from the set t.me/addstickers/FuzzleBot
        .set_default("greeting_sticker_id", "AgADbRIAAhZaEFI")?
        .set_default("periodic_refetch_batch_size", 400)?
        .set_default(
            "default_blacklist",
            vec![
                "meta_sticker".to_string(),
                "gore".to_string(),
                "scat".to_string(),
            ],
        )?
        .add_source(config::File::with_name("./config").required(false))
        .add_source(config::Environment::with_prefix("FUZZLE"))
        .build()?;

    let config: Config = settings.try_deserialize()?;

    let fmt_layer = tracing_subscriber::fmt::layer();

    let telemetry_layer =
        create_otlp_tracer().map(|t| tracing_opentelemetry::layer().with_tracer(t));

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();

    serve_bot_command(config).await?;

    Ok(())
}

async fn serve_bot_command(config: Config) -> Result<()> {
    let update_listener = UpdateListener::new(config).await?;
    update_listener.setup_buttons().await?;
    update_listener.listen().await
}

fn main() -> anyhow::Result<()> {
    with_enough_stack(init())
}
