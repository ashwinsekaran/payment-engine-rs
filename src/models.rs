use anyhow::{anyhow, ensure, Result};
use serde::Deserialize;

pub const SCALE: i64 = 10_000;
pub type Amount = i64;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize)]
pub struct CsvTransaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Account {
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    pub fn total(self) -> Amount {
        self.available + self.held
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            available: 0,
            held: 0,
            locked: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Amount,
    pub disputed: bool,
    pub chargebacked: bool,
}

pub fn parse_amount(raw: &str) -> Result<Amount> {
    let value = raw.trim();
    ensure!(!value.is_empty(), "amount is empty");
    ensure!(!value.starts_with('-'), "amount cannot be negative");

    let (whole, frac) = value
        .split_once('.')
        .map_or((value, ""), |(whole, frac)| (whole, frac));

    ensure!(whole.chars().all(|c| c.is_ascii_digit()), "invalid amount");
    ensure!(frac.chars().all(|c| c.is_ascii_digit()), "invalid amount");
    ensure!(frac.len() <= 4, "amount precision exceeds 4 decimals");

    let whole = if whole.is_empty() {
        0_i128
    } else {
        whole.parse::<i128>()?
    };

    let mut frac_scaled = 0_i128;
    if !frac.is_empty() {
        let frac_value = frac.parse::<i128>()?;
        let pad = 4_u32 - frac.len() as u32;
        frac_scaled = frac_value * 10_i128.pow(pad);
    }

    let scaled = whole
        .checked_mul(SCALE as i128)
        .and_then(|v| v.checked_add(frac_scaled))
        .ok_or_else(|| anyhow!("amount overflow"))?;

    let amount = i64::try_from(scaled)?;
    ensure!(amount > 0, "amount must be greater than zero");

    Ok(amount)
}

pub fn format_amount(amount: Amount) -> String {
    let sign = if amount < 0 { "-" } else { "" };
    let absolute = amount.abs();
    let whole = absolute / SCALE;
    let frac = absolute % SCALE;
    format!("{sign}{whole}.{frac:04}")
}

#[cfg(test)]
mod tests {
    use super::{format_amount, parse_amount, SCALE};

    #[test]
    fn parse_amount_scales_to_four_decimals() {
        assert_eq!(parse_amount("1").unwrap(), SCALE);
        assert_eq!(parse_amount("1.2").unwrap(), 12_000);
        assert_eq!(parse_amount("1.2345").unwrap(), 12_345);
    }

    #[test]
    fn format_amount_always_outputs_four_decimals() {
        assert_eq!(format_amount(0), "0.0000");
        assert_eq!(format_amount(15_000), "1.5000");
        assert_eq!(format_amount(-500), "-0.0500");
    }
}
