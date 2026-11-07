use crate::error::Result;
use std::path::Path;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// File + console logging. The file lives beside the database so a support
/// request can include it. Secrets are never logged: providers redact tokens
/// before they reach a message.
pub fn init(data_dir: &Path, level: &str) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "bt-farm.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);
    let console = tracing_subscriber::fmt::layer().with_target(false).compact();
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(console)
        .try_init();
    Ok(guard)
}

pub fn log_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("logs")
}
