use clap::Parser;
use nutrition_rs::cli::{Cli, run_cli};

#[tokio::main]
async fn main() {
    run_cli(Cli::parse()).await;
}
