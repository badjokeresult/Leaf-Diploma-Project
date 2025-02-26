use clap::Parser;
use clap_derive::Parser;
use clap_derive::Subcommand;
use log::info;
use std::error::Error;

mod platform;
mod server;
mod socket;
mod stor;

#[derive(Parser)]
#[command(name = "leaf-server")]
#[command(about = "Leaf Server Daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    Install {
        #[arg(short, long, default_value = "/etc/systemd/system/leaf-server.service")]
        path: String,
    },
    Uninstall,
    Start,
    Stop,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    std::env::set_var("LEAF_LOG", cli.log_level);
    env_logger::init();

    match &cli.command {
        Some(Commands::Install { path }) => {
            info!("Installing systemd unit file in {}", path);
            platform::install_service(path)?;
            info!("Systemd unit file installed successfully");
            return Ok(());
        }
        Some(Commands::Uninstall) => {
            info!("Removing the service");
            platform::uninstall_service()?;
            return Ok(());
        }
        Some(Commands::Start) => {
            info!("Starting the service");
            platform::start_service()?;
            return Ok(());
        }
        Some(Commands::Stop) => {
            info!("Stopping the service");
            platform::stop_service()?;
            return Ok(());
        }
        None => {
            #[cfg(windows)]
            {
                if let Err(e) = platform::run_as_service() {
                    info!(
                        "Error launching as service: {}, continuing as common app",
                        e
                    );
                } else {
                    return Ok(());
                }
            }

            info!("Launching server in default mode");
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            platform::setup_signal_handler(shutdown_tx)?;
            server::run(shutdown_rx).await?;
        }
    }

    Ok(())
}
