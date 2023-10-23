use anyhow::anyhow;
use clap::Parser;
use docker_volume::handler::VolumeHandler;

use docker_volume_xetfs::command::{SocketType, VolumePluginCommand};
use docker_volume_xetfs::log;
use docker_volume_xetfs::xet_driver::XetDriver;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log::initialize_tracing_subscriber()?;

    let args = VolumePluginCommand::parse();
    let driver = XetDriver::new(args.mount_root);

    let handler = VolumeHandler::new(driver);
    match args.socket_type {
        SocketType::Tcp(tcp_args) => {
            handler.run_tcp(tcp_args.port).await?;
        }
        SocketType::Unix(unix_args) => {
            handler.run_unix_socket(unix_args.socket_path).await?;
        }
        _ => {
            Err(anyhow!("socket type: {:?} unsupported", args.socket_type))?;
        }
    }
    Ok(())
}
