//! Input/Output operations for transaction processing.
//!
//! This module provides functions for reading transaction data from CSV files
//! and writing account details to standard output in CSV format.

use anyhow::{Context, Result};
use std::fs::File;
use std::io;

use crate::types::Accounts;
use crate::types::Transaction;

/// An iterator over transactions from a CSV file.
///
/// This struct owns the CSV reader and file, allowing transactions to be streamed
/// one at a time without loading the entire file into memory.
pub struct TransactionReader {
    reader: csv::Reader<File>,
    path: String,
    line_num: usize,
}

impl Iterator for TransactionReader {
    type Item = Result<Transaction, anyhow::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.deserialize().next().map(|result| {
            self.line_num += 1;
            result.with_context(|| {
                format!(
                    "Failed to parse record at line {} from: {}",
                    self.line_num + 1,
                    self.path
                )
            })
        })
    }
}

/// Reads and parses a CSV file, returning an iterator over `Transaction` structs.
///
/// This function opens the specified CSV file and returns an iterator that lazily
/// deserializes records into `Transaction` structs using serde. The CSV file is expected
/// to have columns for transaction type, client ID, transaction ID, and amount.
///
/// Transactions are streamed one at a time, allowing processing of large files without
/// loading everything into memory.
///
/// # Arguments
///
/// * `path` - The file path to the CSV file to read
///
/// # Returns
///
/// Returns a `Result` containing an iterator over `Transaction` structs on success, or an error
/// if the file cannot be opened or if the CSV headers cannot be read.
///
/// # Errors
///
/// This function will return an error if:
/// - The file cannot be opened (file not found, permission denied, etc.)
/// - The CSV headers cannot be read
///
/// Note: Individual record parsing errors will be returned when iterating over the result.
pub fn read_transactions_from_file(path: &str) -> Result<TransactionReader> {
    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path))?;
    let reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(file);

    Ok(TransactionReader {
        reader,
        path: path.to_string(),
        line_num: 0,
    })
}

/// Writes account details to stdout in CSV format.
///
/// This function takes a map of accounts, sets the client ID for each account
/// from the map key, and serializes them to CSV format. The output is written
/// to standard output.
///
/// # Arguments
///
/// * `accounts` - A map of client IDs to their account details
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing to stdout fails.
///
/// # Errors
///
/// This function will return an error if:
/// - Serialization of any account record fails
/// - Flushing the output buffer fails
pub fn write_accounts_as_csv_to_stdout(accounts: Accounts) -> Result<()> {
    let mut writer = csv::Writer::from_writer(io::stdout());

    for account in accounts.into_iter().map(|(client_id, mut account)| {
        account.client = client_id;
        account
    }) {
        writer
            .serialize(account)
            .context("Failed to write record to stdout")?;
    }

    writer.flush().context("Failed to flush output to stdout")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TxType;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_input_file_reading() {
        // Test reading transactions from the test-data.csv file
        let reader = read_transactions_from_file("test-data.csv").unwrap();

        let transactions: Vec<Transaction> = reader.map(|result| result.unwrap()).collect();

        // Verify we read all 9 transactions (excluding header)
        assert_eq!(transactions.len(), 9);

        // Verify first deposit transaction
        assert_eq!(transactions[0].tx_type, TxType::Deposit);
        assert_eq!(transactions[0].client, 1);
        assert_eq!(transactions[0].tx, 1);
        assert_eq!(transactions[0].amount, Decimal::from_str("10.0").unwrap());

        // Verify second deposit transaction
        assert_eq!(transactions[1].tx_type, TxType::Deposit);
        assert_eq!(transactions[1].client, 2);
        assert_eq!(transactions[1].tx, 2);
        assert_eq!(transactions[1].amount, Decimal::from_str("10.0").unwrap());

        // Verify dispute transaction (should have amount = 0 for empty/missing amount)
        assert_eq!(transactions[2].tx_type, TxType::Dispute);
        assert_eq!(transactions[2].client, 1);
        assert_eq!(transactions[2].tx, 1);
        assert_eq!(transactions[2].amount, Decimal::ZERO);

        // Verify withdrawal transaction
        assert_eq!(transactions[4].tx_type, TxType::Withdrawal);
        assert_eq!(transactions[4].client, 1);
        assert_eq!(transactions[4].tx, 3);
        assert_eq!(transactions[4].amount, Decimal::from_str("5.0").unwrap());

        // Verify resolve transaction
        assert_eq!(transactions[6].tx_type, TxType::Resolve);
        assert_eq!(transactions[6].client, 1);
        assert_eq!(transactions[6].tx, 1);
        assert_eq!(transactions[6].amount, Decimal::ZERO);

        // Verify chargeback transaction (should have amount = 0 for empty/missing amount)
        assert_eq!(transactions[7].tx_type, TxType::Chargeback);
        assert_eq!(transactions[7].client, 2);
        assert_eq!(transactions[7].tx, 2);
        assert_eq!(transactions[7].amount, Decimal::ZERO);
    }
}
