use std::collections::HashMap;

use crate::models::{parse_amount, Account, CsvTransaction, StoredTransaction, TransactionType};

#[derive(Default)]
pub struct Engine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl Engine {
    pub fn process(&mut self, row: CsvTransaction) {
        match row.tx_type {
            TransactionType::Deposit => self.handle_deposit(row),
            TransactionType::Withdrawal => self.handle_withdrawal(row),
            TransactionType::Dispute => self.handle_dispute(row),
            TransactionType::Resolve => self.handle_resolve(row),
            TransactionType::Chargeback => self.handle_chargeback(row),
        }
    }

    pub fn accounts(&self) -> &HashMap<u16, Account> {
        &self.accounts
    }

    fn account_mut(&mut self, client_id: u16) -> &mut Account {
        self.accounts.entry(client_id).or_default()
    }

    fn handle_deposit(&mut self, row: CsvTransaction) {
        let Some(raw_amount) = row.amount.as_deref() else {
            return;
        };

        let Ok(amount) = parse_amount(raw_amount) else {
            return;
        };

        if self.transactions.contains_key(&row.tx) {
            return;
        }

        let account = self.account_mut(row.client);
        if account.locked {
            return;
        }

        account.available += amount;
        self.transactions.insert(
            row.tx,
            StoredTransaction {
                client: row.client,
                amount,
                disputed: false,
                chargebacked: false,
            },
        );
    }

    fn handle_withdrawal(&mut self, row: CsvTransaction) {
        let Some(raw_amount) = row.amount.as_deref() else {
            return;
        };

        let Ok(amount) = parse_amount(raw_amount) else {
            return;
        };

        let account = self.account_mut(row.client);
        if account.locked || account.available < amount {
            return;
        }

        account.available -= amount;
    }

    fn handle_dispute(&mut self, row: CsvTransaction) {
        let (client, amount) = match self.transactions.get_mut(&row.tx) {
            Some(tx) if tx.client == row.client && !tx.disputed && !tx.chargebacked => {
                tx.disputed = true;
                (tx.client, tx.amount)
            }
            _ => return,
        };

        let account = self.account_mut(client);
        if account.locked {
            return;
        }

        account.available -= amount;
        account.held += amount;
    }

    fn handle_resolve(&mut self, row: CsvTransaction) {
        let (client, amount) = match self.transactions.get_mut(&row.tx) {
            Some(tx) if tx.client == row.client && tx.disputed && !tx.chargebacked => {
                tx.disputed = false;
                (tx.client, tx.amount)
            }
            _ => return,
        };

        let account = self.account_mut(client);
        if account.locked {
            return;
        }

        account.available += amount;
        account.held -= amount;
    }

    fn handle_chargeback(&mut self, row: CsvTransaction) {
        let (client, amount) = match self.transactions.get_mut(&row.tx) {
            Some(tx) if tx.client == row.client && tx.disputed && !tx.chargebacked => {
                tx.disputed = false;
                tx.chargebacked = true;
                (tx.client, tx.amount)
            }
            _ => return,
        };

        let account = self.account_mut(client);
        if account.locked {
            return;
        }

        account.held -= amount;
        account.locked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use crate::models::{CsvTransaction, TransactionType};

    fn row(tx_type: TransactionType, client: u16, tx: u32, amount: Option<&str>) -> CsvTransaction {
        CsvTransaction {
            tx_type,
            client,
            tx,
            amount: amount.map(str::to_string),
        }
    }

    #[test]
    fn deposit_and_withdrawal_update_available_and_total() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 1, 1, Some("2.0")));
        engine.process(row(TransactionType::Withdrawal, 1, 2, Some("1.5")));

        let account = engine.accounts().get(&1).unwrap();
        assert_eq!(account.available, 5_000);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 5_000);
    }

    #[test]
    fn dispute_and_resolve_shift_funds_between_available_and_held() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 1, 1, Some("3.0")));
        engine.process(row(TransactionType::Dispute, 1, 1, None));
        engine.process(row(TransactionType::Resolve, 1, 1, None));

        let account = engine.accounts().get(&1).unwrap();
        assert_eq!(account.available, 30_000);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 30_000);
        assert!(!account.locked);
    }

    #[test]
    fn chargeback_removes_held_funds_and_locks_account() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 2, 10, Some("1.25")));
        engine.process(row(TransactionType::Dispute, 2, 10, None));
        engine.process(row(TransactionType::Chargeback, 2, 10, None));

        let account = engine.accounts().get(&2).unwrap();
        assert_eq!(account.available, 0);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 0);
        assert!(account.locked);
    }

    #[test]
    fn dispute_for_wrong_client_is_ignored() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 1, 7, Some("2.0")));
        engine.process(row(TransactionType::Dispute, 2, 7, None));

        let account = engine.accounts().get(&1).unwrap();
        assert_eq!(account.available, 20_000);
        assert_eq!(account.held, 0);
    }
}
