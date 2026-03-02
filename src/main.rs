mod engine;
mod io;
mod models;

use anyhow::{ensure, Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Get input arguments via CLI
    let args: Vec<String> = std::env::args().collect();
    ensure!(
        args.len() == 2,
        "usage: cargo run -- <transactions.csv> > accounts.csv"
    );

    let path = args
        .get(1)
        .context("missing input csv file path argument")?;

    // Initialize engine and process transactions
    let mut engine = engine::Engine::default();

    // Process transactions from an input csv file
    io::process_transactions_file(path, &mut engine)?;

    // Write output to stdout
    io::write_accounts_file(std::io::stdout(), engine.accounts())?;

    Ok(())
}
