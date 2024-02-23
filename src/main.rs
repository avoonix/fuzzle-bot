#[cfg(feature = "ssr")]
mod server_only_stuff {
    use anyhow::Result;
    use clap::{Parser, Subcommand};

    use futures::Future;
    use fuzzle_bot::{Config, Paths, UpdateListener};
    use tokio::fs::{read_to_string, write, File};

    #[derive(Debug, Parser)]
    #[command(name = "fuzzle-bot")]
    #[command(about = "A telegram bot for tagging stickers", long_about = None)]
    struct Cli {
        #[arg(long, env = "FUZZLE_CACHE_DIR_PATH")]
        cache_dir_path: String,
        #[arg(long, env = "FUZZLE_DB_FILE_PATH")]
        db_file_path: String,
        #[arg(long, env = "FUZZLE_CONFIG_FILE_PATH")]
        config_file_path: String,

        #[command(subcommand)]
        command: Option<Commands>,
    }

    #[derive(Debug, Subcommand)]
    enum Commands {
        Serve,
    }

    /// Rust's default thread stack size of 2MiB doesn't allow sufficient recursion depth.
    pub fn with_enough_stack<T>(fut: impl Future<Output = T> + Send) -> T {
        let stack_size = 10 * 1024 * 1024;

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
        let Cli {
            db_file_path,
            cache_dir_path,
            config_file_path,
            command,
        } = Cli::parse();

        let paths = Paths {
            cache_dir_path,
            db_file_path,
            config_file_path,
        };

        match command {
            Some(Commands::Serve) | None => {
                let file = File::open(&paths.config()).await;
                let config: Config = match file {
                    Ok(_) => {
                        let config: Config =
                            toml::from_str(&read_to_string(paths.config()).await?)?;
                        config
                    }
                    Err(_) => {
                        let config = Config::default();
                        write(paths.config(), toml::to_string(&config)?).await?;
                        config
                    }
                };

                serve_bot_command(config, paths).await?;
            }
        };

        Ok(())
    }

    async fn serve_bot_command(config: Config, paths: Paths) -> Result<()> {
        let update_listener = UpdateListener::new(config, paths).await?;
        update_listener.setup_buttons().await?;
        update_listener.listen().await
    }
}

#[cfg(feature = "ssr")]
fn main() -> anyhow::Result<()> {
    use server_only_stuff::{init, with_enough_stack};
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();
    with_enough_stack(init())
}

// TODO: not needed, remove
#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
    // see optional feature `csr` instead
}
