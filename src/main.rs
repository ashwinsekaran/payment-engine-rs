mod engine;
mod io;
mod models;

use anyhow::{ensure, Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    ensure!(
        args.len() == 2,
        "usage: cargo run -- <transactions.csv> > accounts.csv"
    );

    let path = args
        .get(1)
        .context("missing input csv file path argument")?;

    let mut engine = engine::Engine::default();
    io::process_transactions_file(path, &mut engine)?;
    io::write_accounts_file(std::io::stdout(), engine.accounts())?;

    Ok(())
}
