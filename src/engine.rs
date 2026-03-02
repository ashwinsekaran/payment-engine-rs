use std::collections::HashMap;

use crate::models::{parse_amount, Account, CsvTransaction, StoredTransaction, TransactionType};

/// In-memory transaction engine.
///
/// `accounts` stores current balances per client and `transactions` stores
/// transaction metadata required for dispute/resolve/chargeback handling.
#[derive(Default)]
pub struct Engine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl Engine {
    /// Dispatches a CSV transaction row to the relevant transaction handler.
    ///
    /// The handler enforces business rules and silently ignores invalid operations,
    /// matching the engine's "best effort" processing contract.
    pub fn process(&mut self, row: CsvTransaction) {
        match row.tx_type {
            TransactionType::Deposit => self.handle_deposit(row),
            TransactionType::Withdrawal => self.handle_withdrawal(row),
            TransactionType::Dispute => self.handle_dispute(row),
            TransactionType::Resolve => self.handle_resolve(row),
            TransactionType::Chargeback => self.handle_chargeback(row),
        }
    }

    /// Returns an immutable view of all account states keyed by client id.
    pub fn accounts(&self) -> &HashMap<u16, Account> {
        &self.accounts
    }

    /// Returns a mutable account for the client, creating an empty account on first access.
    fn account_mut(&mut self, client_id: u16) -> &mut Account {
        self.accounts.entry(client_id).or_default()
    }

    /// Applies a `deposit` transaction.
    ///
    /// Rules:
    /// - amount must be present and valid
    /// - transaction id must be unique
    /// - destination account must not be locked
    ///
    /// On success, available balance increases and transaction metadata is stored
    /// for possible dispute lifecycle actions.
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
        // Keep successful transactions so later dispute events can reference them.
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

    /// Applies a `withdrawal` transaction.
    ///
    /// Rules:
    /// - amount must be present and valid
    /// - transaction id must be unique
    /// - account must not be locked
    /// - account must have enough available funds
    ///
    /// On success, available balance decreases and transaction metadata is stored
    /// (withdrawals are disputable in this implementation).
    fn handle_withdrawal(&mut self, row: CsvTransaction) {
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
        if account.locked || account.available < amount {
            return;
        }

        account.available -= amount;
        // Store successful withdrawals too (current business rule: withdrawals are disputable).
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

    /// Applies a `dispute` against a previously stored transaction.
    ///
    /// Rules:
    /// - referenced transaction must exist
    /// - transaction must belong to the same client
    /// - transaction must not already be disputed or chargebacked
    /// - account must not be locked
    ///
    /// On success, amount moves from `available` to `held` and total remains unchanged.
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

        // Dispute moves funds from available to held; total remains unchanged.
        account.available -= amount;
        account.held += amount;
    }

    /// Applies a `resolve` for an active dispute.
    ///
    /// Rules:
    /// - referenced transaction must exist and match client id
    /// - transaction must currently be disputed
    /// - transaction must not be already chargebacked
    /// - account must not be locked
    ///
    /// On success, amount moves from `held` back to `available`.
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

        // Resolve releases held funds back to available; total remains unchanged.
        account.available += amount;
        account.held -= amount;
    }

    /// Applies a `chargeback` for an active dispute.
    ///
    /// Rules:
    /// - referenced transaction must exist and match client id
    /// - transaction must currently be disputed
    /// - transaction must not have been chargebacked already
    /// - account must not be locked
    ///
    /// On success, held funds are permanently removed from account total and
    /// the account is locked against further transactions.
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

        // Chargeback finalizes dispute: remove held funds from total and lock account.
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

    fn assert_account(engine: &Engine, client: u16, available: i64, held: i64, total: i64, locked: bool) {
        let account = engine.accounts().get(&client).unwrap();
        assert_eq!(account.available, available);
        assert_eq!(account.held, held);
        assert_eq!(account.total(), total);
        assert_eq!(account.locked, locked);
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

    #[test]
    fn dispute_and_resolve_work_for_withdrawal_transactions() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 48, 1, Some("100.0")));
        engine.process(row(TransactionType::Withdrawal, 48, 9, Some("10.0")));
        engine.process(row(TransactionType::Dispute, 48, 9, None));

        let account = engine.accounts().get(&48).unwrap();
        assert_eq!(account.available, 800_000);
        assert_eq!(account.held, 100_000);
        assert_eq!(account.total(), 900_000);

        engine.process(row(TransactionType::Resolve, 48, 9, None));
        let account = engine.accounts().get(&48).unwrap();
        assert_eq!(account.available, 900_000);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 900_000);
    }

    #[test]
    fn insufficient_withdrawal_then_dispute_blocks_further_withdrawal_until_resolve() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 7, 1, Some("50.0")));
        engine.process(row(TransactionType::Withdrawal, 7, 2, Some("60.0"))); // ignored
        engine.process(row(TransactionType::Deposit, 7, 3, Some("20.0")));
        engine.process(row(TransactionType::Dispute, 7, 3, None));
        engine.process(row(TransactionType::Withdrawal, 7, 4, Some("55.0"))); // ignored (only 50 available)

        assert_account(&engine, 7, 500_000, 200_000, 700_000, false);

        engine.process(row(TransactionType::Resolve, 7, 3, None));
        engine.process(row(TransactionType::Withdrawal, 7, 5, Some("55.0"))); // succeeds

        assert_account(&engine, 7, 150_000, 0, 150_000, false);
    }

    #[test]
    fn mixed_multi_client_flow_with_noise_and_locking() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 1, 1, Some("100.0")));
        engine.process(row(TransactionType::Deposit, 2, 2, Some("5.0")));
        engine.process(row(TransactionType::Withdrawal, 1, 3, Some("30.0")));
        engine.process(row(TransactionType::Dispute, 1, 3, None));
        engine.process(row(TransactionType::Dispute, 2, 3, None)); // wrong client, ignored
        engine.process(row(TransactionType::Withdrawal, 1, 4, Some("80.0"))); // ignored
        engine.process(row(TransactionType::Resolve, 1, 3, None));
        engine.process(row(TransactionType::Withdrawal, 1, 5, Some("70.0"))); // succeeds

        engine.process(row(TransactionType::Withdrawal, 2, 6, Some("10.0"))); // ignored
        engine.process(row(TransactionType::Dispute, 2, 2, None));
        engine.process(row(TransactionType::Chargeback, 2, 2, None)); // lock client 2
        engine.process(row(TransactionType::Deposit, 2, 7, Some("1.0"))); // ignored because locked

        assert_account(&engine, 1, 0, 0, 0, false);
        assert_account(&engine, 2, 0, 0, 0, true);
    }

    #[test]
    fn duplicate_transaction_ids_are_ignored_for_deposit_and_withdrawal() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 10, 1, Some("10.0")));
        engine.process(row(TransactionType::Deposit, 10, 1, Some("20.0"))); // duplicate tx id
        engine.process(row(TransactionType::Withdrawal, 10, 2, Some("5.0")));
        engine.process(row(TransactionType::Withdrawal, 10, 2, Some("1.0"))); // duplicate tx id

        assert_account(&engine, 10, 50_000, 0, 50_000, false);
    }

    #[test]
    fn resolve_and_chargeback_without_active_dispute_are_ignored() {
        let mut engine = Engine::default();

        engine.process(row(TransactionType::Deposit, 11, 1, Some("10.0")));
        engine.process(row(TransactionType::Resolve, 11, 1, None)); // ignored (not disputed)
        engine.process(row(TransactionType::Chargeback, 11, 1, None)); // ignored (not disputed)
        engine.process(row(TransactionType::Dispute, 11, 1, None));
        engine.process(row(TransactionType::Chargeback, 11, 1, None)); // valid
        engine.process(row(TransactionType::Resolve, 11, 1, None)); // ignored (already charged back)

        assert_account(&engine, 11, 0, 0, 0, true);
    }
}
