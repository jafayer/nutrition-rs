use clap::Parser;
use nutrition_rs::cli::{run_cli, Cli};

#[tokio::main]
async fn main() {
    run_cli(Cli::parse()).await;
}
