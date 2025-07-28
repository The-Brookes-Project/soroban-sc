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
│   └── token
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Prerequisites

- Rust 1.70+
- Soroban CLI v22.0.0+
- Stellar account for deployment

## Installation

1. Clone the repository:

   ```
   git clone git@github.com:The-Brookes-Project/soroban-sc.git
   cd soroban-sc
   ```

2. Build the contract:
   ```
   soroban contract build
   ```

## Usage

### Deploy the Contract

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/verse_token.wasm \
  --source ADMIN \
  --network testnet
```

### Initialize the Token

```bash
stellar contract invoke \
  --id CAC5WE5RFIPZ3V54MDGKDFZTLAD7CKZH36AXYGASBH6IPLLGIO77C7ZE \
  --source-account ADMIN \
  --network testnet \
  -- initialize \
  --name "Verseprop Token" \
  --symbol "VSP" \
  --decimals 6 \
  --total-supply 100000000000 \
  --issuer ISSUER \
  --home-domain "verseprop.com" \
  --admin ADMIN \
  --usdc_price 10000000 \
  --usdc_token CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA
```

### Manage Compliance

```bash
# Set KYC status
stellar contract invoke \
  --id CONTRACT_ID \
  --source-account ADMIN_ACCOUNT \
  --network testnet \
  -- set_kyc_status \
  --admin ADMIN_ACCOUNT \
  --address USER_ACCOUNT \
  --verified true

# Set compliance status
stellar contract invoke \
  --id CONTRACT_ID \
  --source-account ADMIN_ACCOUNT \
  --network testnet \
  -- set_compliance_status \
  --admin ADMIN_ACCOUNT \
  --address USER_ACCOUNT \
  --status Approved
```

### Transfer Tokens

```bash
stellar contract invoke \
  --id CONTRACT_ID \
  --source-account SENDER_ACCOUNT \
  --network testnet \
  -- transfer \
  --from SENDER_ACCOUNT \
  --to RECIPIENT_ACCOUNT \
  --amount 1000000
```

## Configuration Options

The contract supports several configuration options:

- **Authorization Required**: Require issuer approval for accounts to hold tokens
- **Authorization Revocable**: Allow issuer to revoke authorization
- **Clawback Enabled**: Enable regulatory clawback functionality
- **Transfer Restrictions**: Restrict transfers to comply with regulations

## Testing

Run the tests with:

```bash
cargo test
```

The test suite includes:

- Token initialization
- Compliance verification
- Transfer functionality
- Clawback operations

## Security Considerations

This implementation includes security controls but is yet to be conducted an audit
