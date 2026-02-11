use std::collections::HashMap;

use anyhow::Result;
use futures::Future;
use fuzzle_bot::{Config, UpdateListener, setup_observability};
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
                "irrelevant_content".to_string(),
            ],
        )?
        .set_default("is_readonly", false)?
        .add_source(config::File::with_name("./config").required(false))
        .add_source(config::Environment::with_prefix("FUZZLE"))
        .build()?;

    let config: Config = settings.try_deserialize()?;

        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:14317".to_string());

    let pyroscope_url = std::env::var("PYROSCOPE_URL")
        .unwrap_or_else(|_| "http://localhost:4040".to_string());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| "fuzzle-bot".to_string());

    let observability = setup_observability(otlp_endpoint, pyroscope_url, service_name).await?;

    serve_bot_command(config).await?;
    
    drop(observability);
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
