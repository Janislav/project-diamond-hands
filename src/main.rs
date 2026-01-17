//! Transaction processing application.
//!
//! This program reads a CSV file containing financial transactions, processes them
//! according to the transaction processing rules, and outputs account summaries to stdout.
//!
//! # Usage
//!
//! ```bash
//! cargo run -- transactions.csv > accounts.csv
//! ```
//!
//! # Input Format
//!
//! The input CSV file should contain transactions with the following columns:
//! - `type`: Transaction type (deposit, withdrawal, dispute, resolve, chargeback)
//! - `client`: Client ID (u16)
//! - `tx`: Transaction ID (u32)
//! - `amount`: Transaction amount (decimal, up to 4 decimal places)
//!
//! # Output Format
//!
//! The program outputs account summaries to stdout in CSV format with columns:
//! - `client`: Client ID
//! - `availabe`: Available balance
//! - `held`: Held balance (funds under dispute)
//! - `total`: Total balance (available + held)
//! - `locked`: Whether the account is locked (true/false)
//!
//! # Examples
//!
//! Process transactions from a file:
//! ```bash
//! cargo run -- transactions.csv
//! ```
//!
//! Redirect output to a file:
//! ```bash
//! cargo run -- transactions.csv > accounts.csv
//! ```
use anyhow::Result;
use std::env;

mod engine;
mod io;
mod types;

/// Main entry point for the transaction processing application.
///
/// This function orchestrates the entire transaction processing pipeline:
/// 1. Reads the input file path from command-line arguments
/// 2. Streams and parses transactions from the CSV file
/// 3. Processes transactions to update account states
/// 4. Writes account summaries to stdout in CSV format
///
/// # Arguments
///
/// The program expects a single command-line argument:
/// - `file_path`: Path to the CSV file containing transactions
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - No input file is provided
/// - The file cannot be opened or read
/// - Any transaction fails to parse
/// - Processing encounters an error
/// - Writing to stdout fails
///
/// # Errors
///
/// This function will return an error if:
/// - Missing command-line argument (input file path)
/// - File I/O errors (file not found, permission denied, etc.)
/// - CSV parsing errors (invalid format, type conversion errors, etc.)
/// - Transaction processing errors
/// - Output writing errors
fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        anyhow::bail!("Missing input file!");
    }

    let file_path = &args[1];
    let transactions = io::read_transactions_from_file(file_path)?;
    let accounts = engine::proccess_transactions(transactions)?;

    io::write_accounts_as_csv_to_stdout(accounts)?;

    Ok(())
}
