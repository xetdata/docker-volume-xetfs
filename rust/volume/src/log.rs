use std::io;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const DEFAULT_LOG_LEVEL: &str = "info";

pub fn initialize_tracing_subscriber() -> Result<(), anyhow::Error> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_line_number(true)
        .with_file(true)
        .with_target(false);
    let filter_layer =
        EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(DEFAULT_LOG_LEVEL))?;
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer.json().with_writer(io::stdout))
        .init();
    Ok(())
}
