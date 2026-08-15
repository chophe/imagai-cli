use clap::Parser;

use imagai::cli::{self, Cli};
use imagai::config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let settings = config::Settings::load();
    cli::run(cli, &settings).await
}
