# Comprehensive Contract Documentation

## Contract Overview

The VerseProp Security Token is a regulatory-compliant security token implemented on the Soroban platform. It includes features for KYC verification, compliance status tracking, token transfers with regulatory controls, and direct USDC purchase functionality.

## Storage Architecture

The contract uses a **scalable storage architecture** that separates contract-level and user-level data:

- **Instance Storage**: Metadata, configuration, admin list, USDC balance (small, fixed-size)
- **Persistent Storage**: User balances, KYC status, compliance status (scales with users)

This architecture ensures the contract can support unlimited users without hitting storage limits. See `STORAGE_ARCHITECTURE.md` for details.

## Contract Methods

### Initialization

#### `initialize`

Initializes the token contract with its required parameters.

**Parameters:**

- `name`: String - The name of the token
- `symbol`: String - The token symbol
- `decimals`: u32 - Number of decimal places (max 7)
- `total_supply`: i128 - Total supply of tokens (must be positive)
- `issuer`: Address - Address of the token issuer
- `home_domain`: String - Domain name associated with the token
- `admin`: Address - Initial admin address
- `usdc_price`: i128 - Price in USDC per token (in smallest unit)
- `usdc_token`: Address - USDC token contract address

**Returns:** SecurityToken - The initialized token object

**Notes:**

- Can only be called once
- Total supply is assigned to the issuer
- Both issuer and admin are added to admin list
- Default settings: authorization required, revocable, clawback enabled, transfer restricted

### Token Operations

#### `transfer`

Transfers tokens between addresses with compliance checks.

**Parameters:**

- `from`: Address - Sender address (requires authentication)
- `to`: Address - Recipient address
- `amount`: i128 - Amount to transfer (must be positive)

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Requires authorization from sender
- Only admins can transfer when transfer_restricted is true
- Both addresses must pass compliance checks
- Sender must have sufficient balance

#### `purchase`

Allows direct purchase of tokens with USDC.

**Parameters:**

- `buyer`: Address - Address of token buyer (requires authentication)
- `token_amount`: i128 - Amount of tokens to purchase (must be positive)

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Buyer must be KYC verified and compliance approved
- USDC is transferred from buyer to contract
- Tokens are transferred from issuer to buyer
- Contract tracks accumulated USDC balance

#### `withdraw_usdc`

Admin function to withdraw accumulated USDC from token purchases.

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `amount`: i128 - Amount of USDC to withdraw

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can withdraw
- Amount must be positive and not exceed accumulated balance
- USDC is transferred from contract to admin

### Compliance and Regulatory Controls

#### `set_kyc_status`

Sets KYC verification status for an address.

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `address`: Address - User address to update
- `verified`: bool - KYC verification status

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can set KYC status

#### `set_compliance_status`

Sets compliance status for an address.

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `address`: Address - User address to update
- `status`: ComplianceStatus - New compliance status (Pending/Approved/Rejected/Suspended)

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can set compliance status

#### `clawback`

Executes clawback of tokens (regulatory action).

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `from`: Address - Address to clawback tokens from
- `amount`: i128 - Amount to clawback (must not exceed balance)

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can clawback tokens
- Clawback must be enabled on the token
- Address must have sufficient balance

### Administrative Functions

#### `add_admin`

Adds a new admin to the token.

**Parameters:**

- `admin`: Address - Existing admin address (requires authentication)
- `new_admin`: Address - New admin address to add

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only existing admin can add new admins
- Cannot add address that is already an admin

#### `configure_authorization`

Configures authorization flags for the token.

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `required`: bool - Whether authorization is required for transfers
- `revocable`: bool - Whether authorization is revocable

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can configure authorization settings

#### `set_transfer_restriction`

Sets the transfer restriction flag.

**Parameters:**

- `admin`: Address - Admin address (requires authentication)
- `restricted`: bool - Whether transfers are restricted

**Returns:** Result<(), Error> - Success or error

**Notes:**

- Only admin can set transfer restriction
- When true, only admins can transfer tokens

### View Functions

#### `get_metadata`

Returns token metadata.

**Parameters:** None

**Returns:** TokenMetadata - The token metadata

#### `balance`

Returns the token balance of an address.

**Parameters:**

- `address`: Address - Address to check

**Returns:** i128 - Token balance (0 if no balance)

#### `check_compliance`

Returns the compliance status of an address.

**Parameters:**

- `address`: Address - Address to check

**Returns:** ComplianceStatus - Compliance status (defaults to Pending)

#### `is_kyc_verified`

Returns the KYC verification status of an address.

**Parameters:**

- `address`: Address - Address to check

**Returns:** bool - KYC status (false if not set)

#### `usdc_balance`

Returns the accumulated USDC balance from token purchases.

**Parameters:** None

**Returns:** i128 - USDC balance

#### `token_price`

Returns the token price in USDC.

**Parameters:** None

**Returns:** i128 - Token price in USDC

## Data Structures

### ComplianceStatus (Enum)

- `Pending` - Initial status
- `Approved` - Approved for transfers
- `Rejected` - Rejected for compliance reasons
- `Suspended` - Temporarily suspended

### TokenMetadata (Struct)

Contains basic token information:

- `name`: String
- `symbol`: String
- `decimals`: u32
- `total_supply`: i128
- `issuer`: Address
- `home_domain`: String
- `usdc_price`: i128
- `usdc_token`: Address

## Error Codes

- 1: Invalid amount (must be positive)
- 2: Transfers restricted
- 3: Not authorized as admin for KYC operations
- 4: Not authorized as admin for compliance operations
- 5: Not authorized as admin for clawback
- 6: Clawback not enabled
- 7: Insufficient balance for clawback
- 8: Not authorized as admin for admin operations
- 9: Address is already an admin
- 10: Not authorized as admin for authorization configuration
- 11: Not authorized as admin for transfer restriction
- 12: KYC verification required
- 13: Compliance approval required
- 14: Insufficient balance for transfer
- 15: Invalid purchase amount
- 16: Invalid USDC amount
- 17: Insufficient issuer balance
- 18: Not authorized as admin for USDC withdrawal
- 19: Invalid withdrawal amount
- 20: Insufficient USDC balance in buyer's account
- 21: USDC transfer verification failed during purchase
- 22: Insufficient USDC in contract for withdrawal
- 23: USDC withdrawal verification failed
- 24: Self-transfer not allowed
