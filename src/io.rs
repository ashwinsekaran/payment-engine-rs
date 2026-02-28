use std::{collections::HashMap, fs::File, io::Read, io::Write};

use anyhow::{Context, Result};
use csv::{ReaderBuilder, Trim, WriterBuilder};
use serde::Serialize;

use crate::{
    engine::Engine,
    models::{format_amount, Account, CsvTransaction},
};

pub fn process_csv_file(path: &str, engine: &mut Engine) -> Result<()> {
    let file = File::open(path).with_context(|| format!("failed to open input file: {path}"))?;
    process_csv_reader(file, engine)
}

pub fn process_csv_reader<R: Read>(reader: R, engine: &mut Engine) -> Result<()> {
    let mut csv_reader = ReaderBuilder::new().trim(Trim::All).from_reader(reader);

    for (index, row) in csv_reader.deserialize::<CsvTransaction>().enumerate() {
        let transaction = row.with_context(|| format!("invalid CSV row at line {}", index + 2))?;
        engine.process(transaction);
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct AccountRow {
    client: u16,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

pub fn write_accounts_csv<W: Write>(writer: W, accounts: &HashMap<u16, Account>) -> Result<()> {
    let mut account_ids: Vec<u16> = accounts.keys().copied().collect();
    account_ids.sort_unstable();

    let mut csv_writer = WriterBuilder::new().has_headers(true).from_writer(writer);
    for client_id in account_ids {
        let account = accounts
            .get(&client_id)
            .expect("account id should exist after collecting keys");

        csv_writer.serialize(AccountRow {
            client: client_id,
            available: format_amount(account.available),
            held: format_amount(account.held),
            total: format_amount(account.total()),
            locked: account.locked,
        })?;
    }

    csv_writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::engine::Engine;

    use super::{process_csv_reader, write_accounts_csv};

    #[test]
    fn processes_csv_and_formats_output_with_four_decimals() {
        let input = r#"type, client, tx, amount
deposit,1,1,2.0
withdrawal,1,2,1.5
deposit,2,3,3.0
dispute,2,3,
chargeback,2,3,
"#;

        let mut engine = Engine::default();
        process_csv_reader(input.as_bytes(), &mut engine).unwrap();

        let mut out = Vec::new();
        write_accounts_csv(&mut out, engine.accounts()).unwrap();
        let output = String::from_utf8(out).unwrap();

        assert!(output.contains("client,available,held,total,locked"));
        assert!(output.contains("1,0.5000,0.0000,0.5000,false"));
        assert!(output.contains("2,0.0000,0.0000,0.0000,true"));
    }
}
