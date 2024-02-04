use anyhow::Result;
use clap::{Parser, Subcommand};

use futures::Future;
use fuzzle_bot::{Config, UpdateListener};

use std::env;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "fuzzle-bot")]
#[command(about = "A telegram bot for tagging stickers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Serve {
        #[arg(long, env = "FUZZLE_TAG_DIR_PATH")]
        tag_dir_path: String,
        #[arg(long, env = "FUZZLE_DB_FILE_PATH")]
        db_file_path: String,
        #[arg(long, env = "FUZZLE_CONFIG_FILE_PATH")]
        config_file_path: String,
    },
}

fn main() -> Result<()> {
    env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();
    with_enough_stack(init())
}

/// Rust's default thread stack size of 2MiB doesn't allow sufficient recursion depth.
fn with_enough_stack<T>(fut: impl Future<Output = T> + Send) -> T {
    let stack_size = 10 * 1024 * 1024;

    // Stack frames are generally larger in debug mode.
    #[cfg(debug_assertions)]
    let stack_size = stack_size * 2;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(stack_size)
        .build()
        .unwrap()
        .block_on(fut)
}

async fn init() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Serve {
            db_file_path,
            tag_dir_path,
            config_file_path,
        } => {
            let config_file_path: PathBuf = config_file_path.into();

            let file = std::fs::File::open(&config_file_path);
            let config: Config = match file {
                Ok(_) => {
                    let config: Config =
                        toml::from_str(&std::fs::read_to_string(config_file_path)?)?;
                    config
                }
                Err(_) => {
                    let config = Config::default();
                    std::fs::write(config_file_path, toml::to_string(&config)?)?;
                    config
                }
            };

            serve_bot_command(config, db_file_path, tag_dir_path).await?;
        }
     
    };

    Ok(())
}


async fn serve_bot_command(
    config: Config,
    db_file_path: String,
    tag_dir_path: String,
) -> Result<()> {
    let update_listener = UpdateListener::new(config, tag_dir_path.into(), db_file_path.into()).await?;
    update_listener.setup_buttons().await?;
    update_listener.listen().await
}
