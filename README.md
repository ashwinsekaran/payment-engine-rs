# payment-engine-rs

Minimal toy payments engine for the Kraken coding challenge.

## Run

```bash
cargo run -- transactions.csv > accounts.csv
```

The binary accepts exactly one argument: the input CSV path.

## Tech used

- `tokio` async runtime (`#[tokio::main]`)
- `axum` routing (`/health` router construction in `src/http.rs`)
- `anyhow` error handling
- `csv` for CSV parsing and writing
- `serde` for CSV row serialization/deserialization

## Project structure

- `src/main.rs`: CLI entrypoint and wiring
- `src/models.rs`: core data models + fixed-point amount parse/format helpers
- `src/engine.rs`: transaction rules (deposit, withdrawal, dispute, resolve, chargeback)
- `src/io.rs`: CSV read/process/write helpers
- `src/http.rs`: minimal axum router

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

## Sample CSV checks

Sample cases are in `samples/`:

- `basic_input.csv` -> `basic_expected.csv`
- `dispute_resolve_chargeback_input.csv` -> `dispute_resolve_chargeback_expected.csv`
- `ignore_invalid_ops_input.csv` -> `ignore_invalid_ops_expected.csv`

Run checks:

```bash
cargo run -- samples/basic_input.csv > /tmp/basic_out.csv && diff -u samples/basic_expected.csv /tmp/basic_out.csv
cargo run -- samples/dispute_resolve_chargeback_input.csv > /tmp/dispute_out.csv && diff -u samples/dispute_resolve_chargeback_expected.csv /tmp/dispute_out.csv
cargo run -- samples/ignore_invalid_ops_input.csv > /tmp/invalid_out.csv && diff -u samples/ignore_invalid_ops_expected.csv /tmp/invalid_out.csv
```
