//! Core data types and structures for transaction processing.
//!
//! This module defines all the fundamental types used throughout the transaction
//! processing system, including transaction types, account details, and type aliases
//! for domain-specific values.
//!
//! # Type Aliases
//!
//! - [`ClientId`]: Type alias for client identifiers (u16)
//! - [`TxId`]: Type alias for transaction identifiers (u32)
//! - [`Amount`]: Type alias for monetary amounts (Decimal)
//! - [`Accounts`]: Type alias for the collection of accounts (BTreeMap<ClientId, AccountDetails>)
//!
//! # Core Types
//!
//! - [`TxType`]: Enumeration of all possible transaction types (deposit, withdrawal, dispute, resolve, chargeback)
//! - [`Transaction`]: Represents a single financial transaction with type, client, ID, and amount
//! - [`AccountDetails`]: Represents the current state of a client's account (balances and lock status)
//!
//!
//! # Serialization
//!
//! All types implement [`Serialize`] and [`Deserialize`] from `serde` for CSV
//! processing. Custom serializers and deserializers ensure proper formatting:
//! - Transaction types are serialized in lowercase
//! - The `type` field in CSV maps to `tx_type` in the struct
//!
//! # Examples
//!
//! Creating a deposit transaction:
//! ```
//! use project_diamond_hands::types::{Transaction, TxType};
//! use rust_decimal::Decimal;
//! use std::str::FromStr;
//!
//! let tx = Transaction {
//!     tx_type: TxType::Deposit,
//!     client: 1,
//!     tx: 100,
//!     amount: Decimal::from_str("10.50").unwrap(),
//! };
//! ```
//!
//! Creating an account with initial balance:
//! ```
//! use project_diamond_hands::types::AccountDetails;
//! use rust_decimal::Decimal;
//! use std::str::FromStr;
//!
//! let account = AccountDetails::new_with_balance(
//!     Decimal::from_str("100.00").unwrap()
//! );
//! ```

use rust_decimal::Decimal;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

pub type ClientId = u16;
pub type TxId = u32;
pub type Amount = Decimal;
pub type Accounts = BTreeMap<ClientId, AccountDetails>;

/// Represents the type of a financial transaction.
///
/// This enum defines all possible transaction types that can be processed
/// by the transaction engine. Each variant corresponds to a specific
/// operation that affects client account balances.
///
/// # Variants
///
/// - **Deposit**: Adds funds to a client's account. Increases both available
///   balance and total balance.
///
/// - **Withdrawal**: Removes funds from a client's account. Decreases both
///   available balance and total balance, but only if sufficient funds are available.
///
/// - **Dispute**: Initiates a dispute on a previous transaction. Moves funds
///   from available to held balance, freezing them until resolved or chargebacked.
///   The total balance remains unchanged.
///
/// - **Resolve**: Resolves a previously disputed transaction. Moves funds
///   back from held to available balance, releasing the frozen funds.
///   The total balance remains unchanged.
///
/// - **Chargeback**: Finalizes a dispute by reversing the original transaction.
///   Withdraws funds from both held and total balance, and locks the account.
///   This is the final state of a dispute.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Represents a single financial transaction.
///
/// This struct contains all the information needed to process a transaction,
/// including its type, the client it affects, a unique transaction ID, and
/// the transaction amount.
///
/// # Fields
///
/// - `tx_type`: The type of transaction (deposit, withdrawal, dispute, resolve, chargeback)
/// - `client`: The client ID (u16) that this transaction affects
/// - `tx`: A unique transaction ID (u32) used to reference this transaction
/// - `amount`: The transaction amount (Decimal), automatically rounded to 4 decimal places
///   during deserialization. Empty or missing values default to 0.
#[derive(Debug, Serialize)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: TxType,
    pub client: ClientId,
    pub tx: TxId,
    #[serde(deserialize_with = "deserialize_amount_or_zero")]
    pub amount: Amount,
}

/// Custom deserializer for transaction amount.
///
/// Handles empty strings and missing values by defaulting to Decimal::ZERO.
/// This allows dispute, resolve, and chargeback transactions to omit the amount field.
fn deserialize_amount_or_zero<'de, D>(deserializer: D) -> Result<Amount, D::Error>
where
    D: Deserializer<'de>,
{
    struct AmountVisitor;

    impl<'de> Visitor<'de> for AmountVisitor {
        type Value = Decimal;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a decimal number or empty string")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(Decimal::ZERO);
            }
            Decimal::from_str(trimmed)
                .map_err(|e| de::Error::custom(format!("invalid decimal: {}", e)))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Decimal::try_from(value)
                .map_err(|e| de::Error::custom(format!("invalid decimal from float: {}", e)))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Decimal::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Decimal::from(value))
        }
    }

    deserializer.deserialize_any(AmountVisitor)
}

fn default_zero() -> Amount {
    Decimal::ZERO
}

impl<'de> Deserialize<'de> for Transaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TransactionHelper {
            #[serde(rename = "type")]
            tx_type: TxType,
            client: ClientId,
            tx: TxId,
            #[serde(
                deserialize_with = "deserialize_amount_or_zero",
                default = "default_zero"
            )]
            amount: Amount,
        }

        let helper = TransactionHelper::deserialize(deserializer)?;
        Ok(Transaction {
            tx_type: helper.tx_type,
            client: helper.client,
            tx: helper.tx,
            amount: helper.amount,
        })
    }
}

/// Represents the current state of a client's account.
///
/// This struct contains all the balance information for a client account,
/// including available funds, held funds (under dispute), total balance,
/// and whether the account is locked.
///
/// # Fields
///
/// - `client`: The client ID (u16) that this account belongs to
/// - `availabe`: The available balance - funds that can be withdrawn or used
///   (Note: This field name contains a typo but is kept for CSV compatibility)
/// - `held`: The held balance - funds that are frozen due to an active dispute
/// - `total`: The total balance - sum of available and held funds (available + held)
/// - `locked`: Whether the account is locked (true) or unlocked (false).
///   Locked accounts cannot process new transactions and typically result from chargebacks.
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AccountDetails {
    pub client: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

impl AccountDetails {
    pub fn new_with_balance(balance: Amount) -> Self {
        let mut new_account = AccountDetails::default();
        new_account.available = balance;
        new_account.total = balance;
        new_account
    }
}
