# Project Diamond Hands

A transaction processing engine written in Rust that processes financial transactions from CSV files and generates account summaries.

## Assumptions

The following assumptions are made:

- The input file contains only valid data. Invalid data is not handled gracefully and will result in an error shutdown.
- Input amounts have the correct level of precision (up to 4 decimal places).
- Withdrawals are not disputable since the money has already left the system.
- After a chargeback the account is marked as locked (frozen). In this implementation, locked accounts ignore any subsequent transactions to prevent further state changes.

## Features

- **Streaming Processing**: Efficiently processes large CSV files without loading everything into memory
- **Precise Decimal Arithmetic**: Uses `rust_decimal` to avoid floating-point precision issues
- **4 Decimal Place Precision**: Automatically supports whatever percision is used in the input data
- **Comprehensive Transaction Support**: Handles deposits, withdrawals, disputes, resolves, and chargebacks
- **Account State Management**: Tracks available, held, and total balances for each client
- **Error Handling**: Robust error handling with detailed error messages

## Tradeoffs/Limitations

- The historical transactions (deposits) are saved in memory instead of being stored in a database. This could grow in memory and ran out of RAM, even though I tried to only save the relevant pieces of data.

- Accounts are also held in memory and could potentially crash the RAM (in theory).

- Streaming is only implemented for reading the input file and processing transactions. The output cannot be streamed because the final state of all accounts is required before generating the CSV output.

## Testing/Validation

- **Input Validation**: Automatic type checking and CSV serialization/deserialization validation
- **Unit Testing**: Business logic is implemented as pure functions and thoroughly tested with unit tests
- **Manual Test Data**: A simple test dataset in **test-data.csv** for manual verification
- **Large Dataset Testing**: Generated **test-data-big.csv** with ~1000 transactions for testing against larger random datasets

## Installation

Clone the repository and build:

```bash
git clone <repository-url>
cd project-diamond-hands
cargo build
```

## Usage

### Basic Usage

The program reads a CSV file containing transactions and outputs account summaries to stdout.

**Input file format** (`transactions.csv`):
```csv
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
withdrawal,1,3,0.5
```

**Output format** (printed to stdout):
```csv
client,availabe,held,total,locked
1,0.5,0,0.5,false
2,2.0,0,2.0,false
```

Process transactions from a CSV file:

```bash
cargo run -- transactions.csv
```

Redirect output to a file:

```bash
cargo run -- transactions.csv > accounts.csv
```

## Transaction Types

### Deposit
Adds funds to a client's account. Increases both available and total balance. If the account doesn't exist, it will be created automatically.

### Withdrawal
Removes funds from a client's account. Decreases both available and total balance, but only if sufficient funds are available. Withdrawals cannot be disputed because the money leaves the system and is gone.

### Dispute
Initiates a dispute on a previous transaction. Moves funds from available to held balance, freezing them until resolved or chargebacked. The total balance remains unchanged.

### Resolve
Resolves a previously disputed transaction. Moves funds back from held to available balance, releasing the frozen funds. The total balance remains unchanged.

### Chargeback
Finalizes a dispute by reversing the original transaction. Withdraws funds from both held and total balance, and locks the account. This is the final state of a dispute.

## Transaction Flow

### Basic Transactions

1. **Deposit**: Adds funds to a client's account. Increases both available and total balance.
2. **Withdrawal**: Removes funds from a client's account. Decreases both available and total balance, but only if sufficient funds are available.

### Dispute Resolution Flows

The system supports two sequential flows for handling disputes:

1. **Dispute → Resolve**: Dispute is resolved, funds are released back to available
2. **Dispute → Chargeback**: Dispute is finalized, funds are withdrawn and account is locked

## Testing

Run the test suite:

```bash
cargo test
```

Run tests with output:

```bash
cargo test -- --nocapture
```

## Project Structure

```
project-diamond-hands/
├── src/
│   ├── main.rs      # Application entry point
│   ├── engine.rs    # Transaction processing engine
│   ├── io.rs        # CSV input/output operations
│   └── types.rs     # Core data types and structures
├── Cargo.toml       # Project dependencies
└── README.md        # This file
```

## Dependencies

- **serde**: Serialization/deserialization framework
- **csv**: CSV file reading and writing
- **anyhow**: Ergonomic error handling
- **rust_decimal**: Precise decimal arithmetic for financial calculations
