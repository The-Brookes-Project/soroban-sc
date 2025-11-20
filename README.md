# Verseprop Soroban Smart Contract

A regulatory-compliant security token implementation using Soroban, Stellar's smart contract platform. This implementation enables the issuance, management, and trading of security tokens with robust compliance controls.

## Features

- **Security Token Issuance**: Create custom security tokens with configurable parameters
- **Compliance Controls**: Built-in KYC/AML verification and status tracking
- **Flexible Administration**: Multi-admin support for operational flexibility
- **Regulatory Features**: Authorization controls, transfer restrictions, and clawback functionality
- **Extensible Architecture**: Modular design for adding custom compliance requirements

## Technical Architecture

The implementation consists of:

- Token contract with full compliance controls
- Authorization and verification mechanisms
- Event emission for auditability
- Comprehensive test suite

## Project Structure

This repository uses the recommended structure for a Soroban project:

```text
.
├── contracts
│   ├── kyc
│   ├── property
│   └── vault
├── scripts
│   └── deploy.sh
├── Cargo.toml
└── README.md
```

## Prerequisites

- Rust 1.70+
- Soroban CLI v22.0.0+
- Stellar account for deployment (configured in your local environment)

## Installation

1. Clone the repository:

   ```bash
   git clone git@github.com:The-Brookes-Project/soroban-sc.git
   cd soroban-sc
   ```

2. Build the contracts:
   ```bash
   stellar contract build
   ```

## Usage

### Deploy the Contracts

We provide a convenience script to build and deploy all contracts (`kyc`, `vault`, `property`).

**Deploy to Testnet (Default):**
```bash
./scripts/deploy.sh
```

**Deploy to Mainnet:**
```bash
./scripts/deploy.sh mainnet
```

The script will output the Contract IDs for `verse_kyc`, `verse_vault`, and `verse_property`. It also saves them to a file named `.contracts_<network>` (e.g., `.contracts_testnet`).

### Initialize the Property Contract

After deployment, you must initialize the property contract. You will need the Contract IDs from the deployment step and a Stablecoin address (e.g., USDC).

```bash
# Load the contract IDs (optional, or just copy-paste them)
source .contracts_testnet

# Initialize
stellar contract invoke \
  --id $PROPERTY_ID \
  --source-account ADMIN \
  --network testnet \
  -- initialize \
  --admin ADMIN \
  --name "Verseprop Token" \
  --symbol "VSP" \
  --decimals 6 \
  --total_supply 100000000000 \
  --token_price 10000000 \
  --vault_address $VAULT_ID \
  --kyc_address $KYC_ID \
  --stablecoin_address USDC_CONTRACT_ADDRESS
```

> **Note:** Replace `USDC_CONTRACT_ADDRESS` with the actual address of the stablecoin you wish to use (e.g., a mock USDC on testnet).

### Manage Compliance (KYC)

To approve a user via the KYC contract:

```bash
stellar contract invoke \
  --id $KYC_ID \
  --source-account ADMIN \
  --network testnet \
  -- set_status \
  --user USER_ACCOUNT \
  --status 1
```
*(Note: Check the KYC contract source for exact enum values for status)*

### Purchase Tokens

Users can purchase tokens if they are KYC approved and have sufficient stablecoin balance.

```bash
stellar contract invoke \
  --id $PROPERTY_ID \
  --source-account USER \
  --network testnet \
  -- purchase_tokens \
  --buyer USER \
  --token_amount 1000000 \
  --enable_compounding true
```

## Configuration Options

The contract supports several configuration options managed via the `update_roi_config` function.

## Testing

Run the tests with:

```bash
cargo test
```

## Security Considerations

This implementation includes security controls but is yet to be conducted an audit.
