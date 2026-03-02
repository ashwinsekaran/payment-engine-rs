use anyhow::{anyhow, ensure, Result};
use serde::{de::Error as _, Deserialize, Deserializer};

/// Fixed-point scale for 4 decimal places (e.g. 1.0000 = 10_000).
pub const SCALE: i64 = 10_000;
/// Internal integer representation for monetary values.
pub type Amount = i64;

/// Supported transaction types from the input CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl<'de> Deserialize<'de> for TransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "deposit" => Ok(Self::Deposit),
            "withdrawal" => Ok(Self::Withdrawal),
            "dispute" => Ok(Self::Dispute),
            "resolve" => Ok(Self::Resolve),
            "chargeback" => Ok(Self::Chargeback),
            _ => Err(D::Error::custom(format!(
                "invalid transaction type: {value}"
            ))),
        }
    }
}

/// Raw transaction row deserialized from CSV input.
#[derive(Debug, Deserialize)]
pub struct CsvTransaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<String>,
}

/// Current account balance state for a client.
#[derive(Debug, Clone, Copy)]
pub struct Account {
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    /// Returns `available + held`.
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

/// Stored transaction metadata required for dispute lifecycle operations.
#[derive(Debug, Clone, Copy)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Amount,
    pub disputed: bool,
    pub chargebacked: bool,
}

/// Parses a decimal amount string into 4-decimal fixed-point integer units.
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

/// Formats fixed-point amount into a decimal string with exactly 4 fractional digits.
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
