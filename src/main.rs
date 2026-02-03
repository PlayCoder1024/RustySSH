//! RustySSH Entry Point

use anyhow::Result;
use rustyssh::{logging, App, Config};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config synchronously to get logging settings before app starts
    let config = Config::load_sync().unwrap_or_default();

    // Initialize file-based logging (returns guard that must be kept alive)
    let _logging_guard = logging::init(&config.settings.logging);

    info!("RustySSH start. Version {}", "0.2.0");

    // Run the application
    let mut app = App::new().await?;
    app.run().await?;

    Ok(())
}
