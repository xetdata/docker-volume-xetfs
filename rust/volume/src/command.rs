use clap::{Args, Parser, Subcommand};
use const_format::concatcp;
use git_version::git_version;
use std::path::PathBuf;

/// The current version
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_VERSION: &str = git_version!(
    args = ["--always", "--dirty", "--exclude=*"],
    fallback = "unknown"
);
const VERSION: &str = concatcp!(CURRENT_VERSION, "-", GIT_VERSION);

const DEFAULT_PORT: u16 = 7280;
const MOUNT_ROOT: &str = "/data";
const STATE_STORAGE: &str = "/tmp/state";
const DEFAULT_SOCKET_PATH: &str = "/run/docker/plugins/xethub.sock";

#[derive(Parser, Debug)]
#[clap(version = CURRENT_VERSION, long_version = VERSION, propagate_version = true)]
#[clap(about = "XetHub Docker Volume Plugin", long_about = None)]
pub struct VolumePluginCommand {
    #[clap(subcommand)]
    pub socket_type: SocketType,
    #[clap(long, short, default_value = MOUNT_ROOT)]
    pub mount_root: PathBuf,
    #[clap(long, short, default_value = STATE_STORAGE)]
    pub state_storage: PathBuf,
}

#[derive(Subcommand, Debug)]
#[non_exhaustive]
pub enum SocketType {
    Tcp(TcpArgs),
    Unix(UnixArgs),
}

#[derive(Args, Debug)]
pub struct TcpArgs {
    #[clap(long, short)]
    #[arg(default_value_t = DEFAULT_PORT)]
    pub port: u16,
}

#[derive(Args, Debug)]
pub struct UnixArgs {
    #[clap(long, short, default_value = DEFAULT_SOCKET_PATH)]
    pub socket_path: PathBuf,
}
