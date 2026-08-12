mod cli;
mod config;
mod core;
mod models;
mod provider;
mod tui;
mod utils;
mod web;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let settings = config::Settings::load();
    cli::run(cli, &settings).await
}
