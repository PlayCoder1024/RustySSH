//! RustySSH Entry Point

use anyhow::Result;
use rustyssh::App;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Run the application
    let mut app = App::new().await?;
    app.run().await?;

    Ok(())
}
