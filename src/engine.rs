//! Transaction processing engine for managing client accounts.
//!
//! This module provides the core business logic for processing financial transactions
//! and maintaining account state. It handles deposits, withdrawals, disputes, resolves,
//! and chargebacks according to the transaction processing rules.

use std::collections::{BTreeMap, HashSet};

use crate::types::AccountDetails;
use crate::types::Accounts;
use crate::types::Transaction;
use crate::types::TxId;
use crate::types::TxType;
use anyhow::Result;

/// Processes transactions from an iterator, maintaining account state.
///
/// # Arguments
///
/// * `transactions` - An iterator over transactions to process (can be `Result<Transaction>` for error handling)
///
/// # Returns
///
/// Returns a map of client IDs to their account details after processing all transactions.
/// If any transaction in the iterator is an error, processing stops and the error is returned.
pub fn proccess_transactions<I>(transactions: I) -> Result<Accounts>
where
    I: IntoIterator<Item = Result<Transaction>>,
{
    let mut accounts = Accounts::new();
    let mut deposit_history: BTreeMap<TxId, Transaction> = BTreeMap::new();
    let mut disputed_transactions: HashSet<TxId> = HashSet::new();

    for tx_result in transactions {
        let tx = tx_result?;
        match tx.tx_type {
            TxType::Deposit => {
                match accounts.get_mut(&tx.client) {
                    Some(account) => {
                        account.availabe =
                            account.availabe.checked_add(tx.amount).ok_or_else(|| {
                                anyhow::anyhow!("Overflow in deposit available balance")
                            })?;
                        account.total = account
                            .total
                            .checked_add(tx.amount)
                            .ok_or_else(|| anyhow::anyhow!("Overflow in deposit total balance"))?;
                    }
                    None => {
                        accounts.insert(tx.client, AccountDetails::new_with_balance(tx.amount));
                    }
                }
                deposit_history.insert(tx.tx, tx);
            }
            TxType::Withdrawal => {
                if let Some(account) = accounts.get_mut(&tx.client) {
                    if tx.amount <= account.availabe {
                        account.total = account.total.checked_sub(tx.amount).ok_or_else(|| {
                            anyhow::anyhow!("Underflow in withdrawal total balance")
                        })?;
                        account.availabe =
                            account.availabe.checked_sub(tx.amount).ok_or_else(|| {
                                anyhow::anyhow!("Underflow in withdrawal available balance")
                            })?;
                    }
                }
            }
            TxType::Dispute => {
                if let Some(account) = accounts.get_mut(&tx.client) {
                    if let Some(disputed_tx) = deposit_history.get(&tx.tx) {
                        // Verify the disputed transaction belongs to the same client
                        // and that there are sufficient funds available to dispute
                        if disputed_tx.client == tx.client && account.availabe >= disputed_tx.amount
                        {
                            account.availabe = account
                                .availabe
                                .checked_sub(disputed_tx.amount)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Underflow in dispute available balance")
                                })?;
                            account.held = account
                                .held
                                .checked_add(disputed_tx.amount)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Overflow in dispute held balance")
                                })?;
                            disputed_transactions.insert(tx.tx);
                        }
                    }
                }
            }
            TxType::Resolve => {
                if let Some(account) = accounts.get_mut(&tx.client) {
                    // Only process if deposit exists, belongs to same client, has an active dispute,
                    // and sufficient funds are held
                    if let Some(original) = deposit_history.get(&tx.tx) {
                        if original.client == tx.client
                            && disputed_transactions.contains(&tx.tx)
                            && account.held >= original.amount
                        {
                            account.availabe = account
                                .availabe
                                .checked_add(original.amount)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Overflow in resolve available balance")
                                })?;
                            account.held =
                                account.held.checked_sub(original.amount).ok_or_else(|| {
                                    anyhow::anyhow!("Underflow in resolve held balance")
                                })?;
                            disputed_transactions.remove(&tx.tx);
                        }
                    }
                }
            }
            TxType::Chargeback => {
                if let Some(account) = accounts.get_mut(&tx.client) {
                    // Only process if deposit exists, belongs to same client, has an active dispute,
                    // and sufficient funds are held
                    if let Some(original) = deposit_history.get(&tx.tx) {
                        if original.client == tx.client
                            && disputed_transactions.contains(&tx.tx)
                            && account.held >= original.amount
                        {
                            account.total =
                                account.total.checked_sub(original.amount).ok_or_else(|| {
                                    anyhow::anyhow!("Underflow in chargeback total balance")
                                })?;
                            account.held =
                                account.held.checked_sub(original.amount).ok_or_else(|| {
                                    anyhow::anyhow!("Underflow in chargeback held balance")
                                })?;
                            account.locked = true;
                            disputed_transactions.remove(&tx.tx);
                        }
                    }
                }
            }
        }
    }

    Ok(accounts)
}

/// Convenience function for tests that processes a vector of transactions.
#[cfg(test)]
fn proccess_transactions_vec(transactions: Vec<Transaction>) -> Accounts {
    proccess_transactions(transactions.into_iter().map(Ok)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn withdraw_succeeds_if_suficent_funds() {
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Withdrawal,
                client: 1,
                tx: 2,
                amount: Decimal::from_str("5.0").unwrap(), // Less than available
            },
        ];

        let accounts = proccess_transactions_vec(transactions);

        // Verify the account exists
        let account = accounts.get(&1).expect("Account should exist");

        // Verify the withdrawal succeeded - balance should be 5.0 (10.0 - 5.0)
        assert_eq!(account.availabe, Decimal::from_str("5.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("5.0").unwrap());
    }

    #[test]
    fn withdraw_fails_if_insufficent_funds() {
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Withdrawal,
                client: 1,
                tx: 2,
                amount: Decimal::from_str("15.0").unwrap(), // More than available
            },
        ];

        let accounts = proccess_transactions_vec(transactions);

        // Verify the account exists
        let account = accounts.get(&1).expect("Account should exist");

        // Verify the withdrawal failed - balance should still be 10.0
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn dispute_transacion() {
        // Test successful dispute - funds move from available to held, total unchanged
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1,                 // Disputes transaction 1
                amount: Decimal::ZERO, // Dispute doesn't have an amount
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Available should decrease by disputed amount (10.0)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        // Held should increase by disputed amount (10.0)
        assert_eq!(account.held, Decimal::from_str("10.0").unwrap());
        // Total should remain unchanged
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn dispute_nonexistent_transaction_is_ignored() {
        // Test that disputing a non-existent transaction is ignored
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 999, // Disputes non-existent transaction
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should be unchanged since dispute was ignored
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn dispute_partial_funds() {
        // Test dispute when account has multiple deposits
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 2,
                amount: Decimal::from_str("5.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1, // Disputes first deposit
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Available should be 5.0 (only second deposit remains available)
        assert_eq!(account.availabe, Decimal::from_str("5.0").unwrap());
        // Held should be 10.0 (first deposit is held)
        assert_eq!(account.held, Decimal::from_str("10.0").unwrap());
        // Total should be 15.0 (sum of both deposits)
        assert_eq!(account.total, Decimal::from_str("15.0").unwrap());
    }

    #[test]
    fn resolve_transaction() {
        // Test successful resolve - funds move from held back to available, total unchanged
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1, // Disputes transaction 1
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 1,                 // Resolves transaction 1
                amount: Decimal::ZERO, // Resolve doesn't have an amount
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // After resolve, funds should be back in available
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        // Held should be back to zero
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        // Total should remain unchanged
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn resolve_nonexistent_transaction_is_ignored() {
        // Test that resolving a non-existent transaction is ignored
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1,
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 999, // Resolves non-existent transaction
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should still have funds in held (resolve was ignored)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn resolve_transaction_without_dispute_is_ignored() {
        // Test that resolving a transaction that isn't disputed is ignored
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            // No dispute for transaction 1
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 1, // Tries to resolve transaction 1 (but it's not disputed)
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should be unchanged (resolve was ignored)
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
    }

    #[test]
    fn resolve_after_chargeback_is_ignored() {
        // Test that resolving a transaction that was chargebacked is ignored
        // (since chargeback withdraws the held funds, there's nothing to resolve)
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1,
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 1, // Chargebacks the dispute (funds withdrawn, account locked)
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 1, // Tries to resolve (but funds already withdrawn, nothing in held)
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should be as if resolve never happened (funds withdrawn, account locked)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("0.0").unwrap());
        // Account should still be locked (chargeback happened, resolve was ignored)
        assert!(
            account.locked,
            "Account should be locked after chargeback, resolve was ignored"
        );
    }

    #[test]
    fn resolve_partial_funds() {
        // Test resolve when account has multiple disputed transactions
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 2,
                amount: Decimal::from_str("5.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1, // Disputes first deposit
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 2, // Disputes second deposit
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 1, // Resolves first deposit only
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Available should be 10.0 (first deposit resolved)
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        // Held should be 5.0 (second deposit still disputed)
        assert_eq!(account.held, Decimal::from_str("5.0").unwrap());
        // Total should be 15.0 (sum of both deposits)
        assert_eq!(account.total, Decimal::from_str("15.0").unwrap());
    }

    #[test]
    fn chargeback_transacion() {
        // Test successful chargeback - funds withdrawn from held and total, account locked
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1, // Disputes transaction 1
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 1,                 // Chargebacks transaction 1
                amount: Decimal::ZERO, // Chargeback doesn't have an amount
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Available should remain 0 (was moved to held, then withdrawn)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        // Held should be 0 (withdrawn)
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        // Total should decrease by disputed amount (10.0 - 10.0 = 0.0)
        assert_eq!(account.total, Decimal::from_str("0.0").unwrap());
        // Account should be locked
        assert!(account.locked, "Account should be locked after chargeback");
    }

    #[test]
    fn chargeback_nonexistent_transaction_is_ignored() {
        // Test that chargebacking a non-existent transaction is ignored
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1,
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 999, // Chargebacks non-existent transaction
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should still have funds in held (chargeback was ignored)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
        // Account should not be locked
        assert!(
            !account.locked,
            "Account should not be locked when chargeback is ignored"
        );
    }

    #[test]
    fn chargeback_transaction_without_dispute_is_ignored() {
        // Test that chargebacking a transaction that isn't disputed is ignored
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            // No dispute for transaction 1
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 1, // Tries to chargeback transaction 1 (but it's not disputed)
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should be unchanged (chargeback was ignored)
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
        // Account should not be locked
        assert!(
            !account.locked,
            "Account should not be locked when chargeback is ignored"
        );
    }

    #[test]
    fn chargeback_partial_funds() {
        // Test chargeback when account has multiple disputed transactions
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 2,
                amount: Decimal::from_str("5.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1, // Disputes first deposit
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 2, // Disputes second deposit
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 1, // Chargebacks first deposit only
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Available should be 0 (first deposit was disputed, then chargebacked)
        assert_eq!(account.availabe, Decimal::from_str("0.0").unwrap());
        // Held should be 5.0 (second deposit still disputed)
        assert_eq!(account.held, Decimal::from_str("5.0").unwrap());
        // Total should be 5.0 (first deposit withdrawn: 15.0 - 10.0 = 5.0)
        assert_eq!(account.total, Decimal::from_str("5.0").unwrap());
        // Account should be locked
        assert!(account.locked, "Account should be locked after chargeback");
    }

    #[test]
    fn chargeback_after_resolve_is_ignored() {
        // Test that chargebacking a transaction that was resolved is ignored
        // (since resolve releases the held funds, there's no active dispute to chargeback)
        let transactions = vec![
            Transaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::from_str("10.0").unwrap(),
            },
            Transaction {
                tx_type: TxType::Dispute,
                client: 1,
                tx: 1,
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Resolve,
                client: 1,
                tx: 1, // Resolves the dispute (funds back to available)
                amount: Decimal::ZERO,
            },
            Transaction {
                tx_type: TxType::Chargeback,
                client: 1,
                tx: 1, // Tries to chargeback (but dispute was resolved, no funds held)
                amount: Decimal::ZERO,
            },
        ];

        let accounts = proccess_transactions_vec(transactions);
        let account = accounts.get(&1).expect("Account should exist");

        // Account should be as if chargeback never happened (funds back in available)
        assert_eq!(account.availabe, Decimal::from_str("10.0").unwrap());
        assert_eq!(account.held, Decimal::from_str("0.0").unwrap());
        assert_eq!(account.total, Decimal::from_str("10.0").unwrap());
        // Account should not be locked (chargeback was ignored)
        assert!(
            !account.locked,
            "Account should not be locked when chargeback is ignored"
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::types::{Transaction, TxType};
    use proptest::prelude::*;
    use rust_decimal::Decimal;

    /// Generates a strategy for creating random transactions.
    ///
    /// This generates deposits, withdrawals, disputes, resolves, and chargebacks with:
    /// - Client IDs: 1-10
    /// - Transaction IDs: 1-1000
    /// - Amounts: 0.01 to 1000.0 (rounded to 0-4 decimal places for deposits/withdrawals)
    /// - Disputes/resolves/chargebacks reference existing deposit transaction IDs
    fn transaction_strategy() -> impl Strategy<Value = Vec<Transaction>> {
        prop::collection::vec(
            (
                1u16..=10u16,       // client
                1u32..=1000u32,     // tx
                (1u64..=100000u64), // amount in cents (0.01 to 1000.00)
                0u8..=9u8,          // transaction type selector
                0u8..=3u8,          // decimal places (0-4)
            ),
            1..=100,
        )
        .prop_map(|tx_params| {
            let mut transactions = Vec::new();
            let mut deposit_tx_ids: Vec<(u16, u32, Decimal)> = Vec::new(); // (client, tx_id, amount)
            let mut tx_id_counter = 1u32;

            for (client, _tx_id, amount_cents, tx_type_selector, decimal_places) in tx_params {
                // Convert amount to decimal with variable precision
                let mut amount = Decimal::from(amount_cents) / Decimal::from(100);
                amount = amount.round_dp(decimal_places as u32);

                let tx = match tx_type_selector {
                    0..=4 => {
                        // 50% deposits
                        let deposit_tx = Transaction {
                            tx_type: TxType::Deposit,
                            client,
                            tx: tx_id_counter,
                            amount,
                        };
                        deposit_tx_ids.push((client, tx_id_counter, amount));
                        tx_id_counter += 1;
                        deposit_tx
                    }
                    5..=6 => {
                        // 20% withdrawals
                        let withdrawal_tx = Transaction {
                            tx_type: TxType::Withdrawal,
                            client,
                            tx: tx_id_counter,
                            amount,
                        };
                        tx_id_counter += 1;
                        withdrawal_tx
                    }
                    7 => {
                        // 10% disputes (reference existing deposit)
                        if let Some((ref_client, ref_tx_id, _)) = deposit_tx_ids.last() {
                            Transaction {
                                tx_type: TxType::Dispute,
                                client: *ref_client,
                                tx: *ref_tx_id,
                                amount: Decimal::ZERO,
                            }
                        } else {
                            // No deposits yet, create a deposit instead
                            let deposit_tx = Transaction {
                                tx_type: TxType::Deposit,
                                client,
                                tx: tx_id_counter,
                                amount,
                            };
                            deposit_tx_ids.push((client, tx_id_counter, amount));
                            tx_id_counter += 1;
                            deposit_tx
                        }
                    }
                    8 => {
                        // 10% resolves (reference existing deposit)
                        if let Some((ref_client, ref_tx_id, _)) = deposit_tx_ids.last() {
                            Transaction {
                                tx_type: TxType::Resolve,
                                client: *ref_client,
                                tx: *ref_tx_id,
                                amount: Decimal::ZERO,
                            }
                        } else {
                            // No deposits yet, create a deposit instead
                            let deposit_tx = Transaction {
                                tx_type: TxType::Deposit,
                                client,
                                tx: tx_id_counter,
                                amount,
                            };
                            deposit_tx_ids.push((client, tx_id_counter, amount));
                            tx_id_counter += 1;
                            deposit_tx
                        }
                    }
                    _ => {
                        // 10% chargebacks (reference existing deposit)
                        if let Some((ref_client, ref_tx_id, _)) = deposit_tx_ids.last() {
                            Transaction {
                                tx_type: TxType::Chargeback,
                                client: *ref_client,
                                tx: *ref_tx_id,
                                amount: Decimal::ZERO,
                            }
                        } else {
                            // No deposits yet, create a deposit instead
                            let deposit_tx = Transaction {
                                tx_type: TxType::Deposit,
                                client,
                                tx: tx_id_counter,
                                amount,
                            };
                            deposit_tx_ids.push((client, tx_id_counter, amount));
                            tx_id_counter += 1;
                            deposit_tx
                        }
                    }
                };

                transactions.push(tx);
            }

            transactions
        })
    }

    /// Property test: After processing any sequence of transactions,
    /// all account balances (available, held, total) must be non-negative.
    #[test]
    fn balance_is_never_negative() {
        proptest!(|(transactions in transaction_strategy())| {
            let accounts = proccess_transactions(transactions.into_iter().map(Ok)).unwrap();

            for (client_id, account) in accounts {
                prop_assert!(
                    account.availabe >= Decimal::ZERO,
                    "Account {} available balance must be non-negative, got {}",
                    client_id,
                    account.availabe
                );
                prop_assert!(
                    account.held >= Decimal::ZERO,
                    "Account {} held balance must be non-negative, got {}",
                    client_id,
                    account.held
                );
                prop_assert!(
                    account.total >= Decimal::ZERO,
                    "Account {} total balance must be non-negative, got {}",
                    client_id,
                    account.total
                );
            }
        });
    }
}
