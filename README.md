# payment-engine-rs

Minimal toy payments engine for simple transactions tracked in CSV built in Rust. 

## Run

```bash
cargo run -- transactions.csv > accounts.csv
```

The binary accepts exactly one argument: the input CSV path.

For this repository structure:

```bash
cargo run -- input/basic_input.csv > output/basic_output.csv
```

## Tech used

- `anyhow` error handling
- `csv` for CSV parsing and writing
- `serde` for CSV row serialization/deserialization
- `tokio` async runtime (`#[tokio::main]`)

## Project structure

- `src/main.rs`: CLI entrypoint and wiring
- `src/models.rs`: core data models + fixed-point amount parse/format helpers
- `src/engine.rs`: transaction rules (deposit, withdrawal, dispute, resolve, chargeback)
- `src/io.rs`: CSV read/process/write helpers

## Behavior notes

- Uses fixed-point math (`i64`, 4 decimal places) to avoid floating-point errors.
- Input is streamed row-by-row from CSV (`csv::Reader`), so the full file is not loaded into memory.
- Invalid or irrelevant state transitions are ignored (e.g., dispute for unknown transaction, resolve for non-disputed transaction).
- After chargeback, account is locked and ignores further transactions.

## Testing

Run:

```bash
cargo test
```

Current tests cover:

- amount parsing/formatting precision
- deposit/withdrawal flow
- dispute/resolve flow
- chargeback lock behavior
- end-to-end CSV processing/output shape

## Input file

- `input/basic_input.csv`: primary input file for running the engine locally.

## Docker

Build image:

```bash
docker build -t payment-engine-rs:latest .
```

Run with input file (prints CSV to stdout):

```bash
docker run --rm -v "$PWD/input:/app/input:ro" payment-engine-rs:latest /app/input/basic_input.csv
```

Run and save output to host file:

```bash
docker run --rm -v "$PWD/input:/app/input:ro" payment-engine-rs:latest /app/input/basic_input.csv > output/basic_output.csv
```
