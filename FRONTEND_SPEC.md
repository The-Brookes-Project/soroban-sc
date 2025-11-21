# Verseprop Frontend Integration Specification

## Overview

This document describes the smart contract integration required for the Verseprop property token platform frontend.

## Contract Addresses (Testnet)

After deployment, you will receive three contract IDs:
- **KYC Contract**: `KYC_ID`
- **Vault Contract**: `VAULT_ID`
- **Property Contract**: `PROPERTY_ID`
- **USDC Token**: `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`

These will be stored in `.contracts_testnet` after running the deployment script.

---

## User Flow 1: Purchase Property Tokens

### Prerequisites (Backend/Admin Check)
Before a user can purchase, they must be KYC approved:
```typescript
// Check if user is KYC verified (read-only)
const isVerified = await kycContract.is_kyc_verified({ user: userAddress });
const complianceStatus = await kycContract.get_compliance_status({ user: userAddress });
// complianceStatus should be "Approved"
```

### Step 1: Check User USDC Balance
```typescript
// Get user's USDC balance
const usdcBalance = await usdcContract.balance({ id: userAddress });
// Display in UI: format with 7 decimals (e.g., 10000000 = 1 USDC)
```

### Step 2: Get Property Details
```typescript
// Get property metadata
const metadata = await propertyContract.get_metadata();
/*
Returns: {
  name: "Verseprop Test Token",
  symbol: "VPT",
  decimals: 7,
  total_supply: 10000000000000,
  token_price: 10000000,  // Price per token in USDC (7 decimals)
  vault_address: "VAULT_ID",
  kyc_address: "KYC_ID",
  stablecoin_address: "USDC_ID"
}
*/

// Calculate cost
const tokensToBuy = userInput; // e.g., 1000000000 (100 tokens with 7 decimals)
const cost = tokensToBuy * metadata.token_price / Math.pow(10, metadata.decimals);
// Display: "Cost: {cost} USDC"
```

### Step 3: Purchase Tokens
```typescript
// User chooses whether to enable compounding
const enableCompounding = true; // or false based on user selection

// Execute purchase
await propertyContract.purchase_tokens({
  buyer: userAddress,
  token_amount: tokensToBuy,
  enable_compounding: enableCompounding
});

// This will:
// 1. Transfer USDC from user to property contract
// 2. Create a position for the user
// 3. Start the epoch timer (30 days or 5 minutes for testing)
```

### Step 4: Show Confirmation
After purchase, display:
- Tokens purchased
- Cost in USDC
- Position created
- Epoch start time
- Expected epoch end time (start + 30 days)

---

## User Flow 2: View Position Details

### Get User Position
```typescript
const position = await propertyContract.get_user_position({ user: userAddress });
/*
Returns: {
  tokens: 1000000000,
  initial_investment: 10000000,
  current_principal: 10000000,
  compounding_enabled: true,
  epoch_start: 1700000000,
  consecutive_rollovers: 0,
  total_yield_earned: 0,
  loyalty_tier: 0
}
or null if no position exists
*/
```

### Display Position Information
```typescript
if (position) {
  // Format for display (assuming 7 decimals)
  const tokensFormatted = position.tokens / 10000000;
  const principalFormatted = position.current_principal / 10000000;
  const yieldFormatted = position.total_yield_earned / 10000000;
  
  // Calculate time to expiry
  const currentTime = Math.floor(Date.now() / 1000);
  const epochDuration = 2592000; // 30 days in seconds (or 300 for testing)
  const epochEnd = position.epoch_start + epochDuration;
  const timeRemaining = epochEnd - currentTime;
  
  // Display:
  // - Tokens: {tokensFormatted} VPT
  // - Current Principal: {principalFormatted} USDC
  // - Total Yield Earned: {yieldFormatted} USDC
  // - Compounding: {position.compounding_enabled ? 'Enabled' : 'Disabled'}
  // - Loyalty Tier: {position.loyalty_tier}
  // - Time to Epoch End: {formatSeconds(timeRemaining)}
  // - Consecutive Rollovers: {position.consecutive_rollovers}
}
```

### Preview Yield
```typescript
const yieldPreview = await propertyContract.preview_yield({ user: userAddress });
/*
Returns: {
  base_yield: 666666,
  compounding_bonus: 166666,
  loyalty_bonus: 0,
  total_yield: 833332,
  days_elapsed: 15,
  days_remaining: 15
}
*/

// Display preview:
// - Base Yield: {base_yield / 10000000} USDC
// - Compounding Bonus: {compounding_bonus / 10000000} USDC
// - Loyalty Bonus: {loyalty_bonus / 10000000} USDC
// - Total Yield (this epoch): {total_yield / 10000000} USDC
// - Progress: {days_elapsed} / 30 days
```

### Check If User Can Take Action
```typescript
const canTakeAction = await propertyContract.can_take_action({ user: userAddress });
// Returns: true if epoch is complete, false otherwise

const isInGracePeriod = await propertyContract.is_in_grace_period({ user: userAddress });
// Returns: true if in 24-hour grace period after epoch end

// Display:
// If canTakeAction: "You can now rollover or liquidate your position"
// If isInGracePeriod: "Grace period active - you have 24 hours to take action"
// Otherwise: Show countdown to epoch end
```

---

## User Flow 3: Rollover Position

### Prerequisites
- Epoch must be complete (`can_take_action` returns true)

### Execute Rollover
```typescript
await propertyContract.rollover_position({ user: userAddress });

// This will:
// 1. Calculate yield for completed epoch
// 2. If compounding enabled: add yield to principal
// 3. If compounding disabled: track yield separately
// 4. Increment loyalty tier (up to max 4)
// 5. Reset epoch timer
// 6. Increment consecutive_rollovers counter
```

### Show Rollover Results
```typescript
// Fetch updated position
const updatedPosition = await propertyContract.get_user_position({ user: userAddress });

// Display:
// - Previous Principal: {oldPrincipal} USDC
// - Yield Earned This Epoch: {yieldEarned} USDC
// - New Principal: {updatedPosition.current_principal / 10000000} USDC (if compounding)
// - Total Yield Earned: {updatedPosition.total_yield_earned / 10000000} USDC
// - New Loyalty Tier: {updatedPosition.loyalty_tier}
// - Consecutive Rollovers: {updatedPosition.consecutive_rollovers}
// - New Epoch Started
```

---

## User Flow 4: Liquidate Position

### Prerequisites
- Epoch must be complete (`can_take_action` returns true)

### Execute Liquidation
```typescript
// Get position before liquidation
const positionBefore = await propertyContract.get_user_position({ user: userAddress });
const usdcBalanceBefore = await usdcContract.balance({ id: userAddress });

// Execute liquidation
await propertyContract.liquidate_position({ user: userAddress });

// This will:
// 1. Calculate final yield for current epoch
// 2. Calculate total payout (principal + final yield)
// 3. Request liquidation from vault
// 4. Vault transfers USDC to user
// 5. Remove position from storage
// 6. Update total active tokens

// Get balances after
const usdcBalanceAfter = await usdcContract.balance({ id: userAddress });
const positionAfter = await propertyContract.get_user_position({ user: userAddress });
// positionAfter will be null
```

### Show Liquidation Results
```typescript
const receivedAmount = usdcBalanceAfter - usdcBalanceBefore;

// Display:
// - Position Liquidated
// - Initial Investment: {positionBefore.initial_investment / 10000000} USDC
// - Final Principal: {positionBefore.current_principal / 10000000} USDC
// - Total Yield Earned: {positionBefore.total_yield_earned / 10000000} USDC
// - Payout Received: {receivedAmount / 10000000} USDC
// - New USDC Balance: {usdcBalanceAfter / 10000000} USDC
// - Consecutive Rollovers: {positionBefore.consecutive_rollovers}
```

---

## Vault State Visualization

### Get Vault Configuration
```typescript
const vaultConfig = await vaultContract.get_config();
/*
Returns: {
  admin: "ADMIN_ADDRESS",
  stablecoin_address: "USDC_ID",
  total_capacity: 1000000000000,
  available: 800000000000,
  buffer_percentage: 15,
  controlled_mode: false,
  emergency_pause: false
}
*/

// Display:
// - Total Capacity: {total_capacity / 10000000} USDC
// - Available Liquidity: {available / 10000000} USDC
// - Buffer Percentage: {buffer_percentage}%
// - Status: {emergency_pause ? 'Paused' : 'Active'}
// - Mode: {controlled_mode ? 'Controlled (Queue Active)' : 'Normal'}
```

### Get Available Liquidity
```typescript
const availableLiquidity = await vaultContract.available_liquidity();
// Display: "Available Liquidity: {availableLiquidity / 10000000} USDC"
```

### Get Total Capacity
```typescript
const totalCapacity = await vaultContract.total_capacity();
// Display: "Total Vault Capacity: {totalCapacity / 10000000} USDC"
```

### Get Queue Status (if in controlled mode)
```typescript
const queueStatus = await vaultContract.get_queue_status();
/*
Returns: {
  total_queued: 5,
  total_amount: 50000000000,
  controlled_mode: true,
  head_index: 0,
  tail_index: 5,
  estimated_clear_time: 1700086400
}
*/

// Display:
// - Withdrawal Requests in Queue: {total_queued}
// - Total Amount Queued: {total_amount / 10000000} USDC
// - Estimated Processing Time: {formatDate(estimated_clear_time)}
```

---

## ROI Configuration Display

### Get ROI Config
```typescript
const roiConfig = await propertyContract.get_roi_config();
/*
Returns: {
  annual_rate_bps: 800,      // 8% APY
  compounding_bonus_bps: 200, // +2% bonus
  loyalty_bonus_bps: 25,      // 25 bps per tier
  cash_flow_monthly: 10000000000
}
*/

// Display:
// - Base APY: {annual_rate_bps / 100}%
// - Compounding Bonus: +{compounding_bonus_bps / 100}%
// - Loyalty Bonus per Tier: {loyalty_bonus_bps / 100}%
// - Expected Monthly Cash Flow: {cash_flow_monthly / 10000000} USDC
```

---

## Total Active Tokens

### Get Total Active Tokens
```typescript
const totalActive = await propertyContract.total_active_tokens();
// Display: "Total Active Tokens: {totalActive / 10000000} VPT"
```

---

## Dashboard Summary

Here's what to display on the main dashboard:

### User Section (if logged in and has position)
1. **Position Overview**
   - Tokens held
   - Current principal
   - Total yield earned
   - Compounding status
   - Loyalty tier
   
2. **Current Epoch**
   - Days elapsed / Total days
   - Progress bar
   - Estimated yield this epoch
   - Time remaining
   
3. **Actions**
   - Buy More (if available supply)
   - Rollover (if epoch complete)
   - Liquidate (if epoch complete)

### Property Overview Section
1. **Property Details**
   - Property name
   - Token symbol
   - Price per token
   - Total supply
   - Total active tokens
   
2. **Vault Status**
   - Total capacity
   - Available liquidity
   - Liquidity utilization %
   - Queue status (if applicable)
   
3. **ROI Information**
   - Base APY
   - Compounding bonus
   - Max loyalty bonus
   - Your effective APY (based on tier)

---

## Error Handling

Common errors and user-friendly messages:

| Error Message | User-Friendly Message |
|--------------|----------------------|
| "User not KYC verified" | "Please complete KYC verification before purchasing" |
| "User not approved for trading" | "Your account is pending approval" |
| "Insufficient USDC balance" | "Insufficient USDC balance. Please add funds." |
| "Position already exists" | "You already have an active position" |
| "No position found" | "No active position found" |
| "Epoch not complete" | "Please wait for the current epoch to complete" |
| "Insufficient vault balance" | "Vault liquidity temporarily unavailable. Your request has been queued." |
| "Emergency paused" | "The system is temporarily paused. Please try again later." |

---

## WebSocket Events (Optional Enhancement)

If you want real-time updates, listen to contract events:

### Events to Monitor
1. **TokensPurchased** - User bought tokens
2. **PositionRolledOver** - User rolled over
3. **PositionLiquidated** - User liquidated
4. **LiquidationQueued** - Liquidation request queued
5. **LiquidationExecuted** - Queued liquidation processed

---

## Complete Example Flow

```typescript
// 1. User connects wallet
const userAddress = await wallet.connect();

// 2. Check if user has position
const position = await propertyContract.get_user_position({ user: userAddress });

if (!position) {
  // 3a. New user - show purchase flow
  const metadata = await propertyContract.get_metadata();
  const usdcBalance = await usdcContract.balance({ id: userAddress });
  
  // Show purchase UI with:
  // - Token price
  // - Available USDC
  // - Token amount input
  // - Compounding toggle
  
  // Execute purchase
  await propertyContract.purchase_tokens({
    buyer: userAddress,
    token_amount: amount,
    enable_compounding: true
  });
} else {
  // 3b. Existing user - show position dashboard
  const yieldPreview = await propertyContract.preview_yield({ user: userAddress });
  const canTakeAction = await propertyContract.can_take_action({ user: userAddress });
  
  // Show position details and yield preview
  
  if (canTakeAction) {
    // Show rollover and liquidate buttons
  } else {
    // Show countdown to epoch end
  }
}

// 4. Always show vault status
const vaultConfig = await vaultContract.get_config();
const roiConfig = await propertyContract.get_roi_config();

// Display vault and ROI info
```

---

## Notes

1. **Decimals**: All amounts use 7 decimals. Divide by 10,000,000 to get human-readable values.
2. **Epoch Duration**: Currently 30 days (2,592,000 seconds) in production, may be 300 seconds (5 minutes) for testing.
3. **Grace Period**: 24 hours (86,400 seconds) after epoch end.
4. **Loyalty Tiers**: Max 4 tiers, each gives an additional bonus.
5. **Buffer**: Vault maintains a 15% buffer for future liquidations.

---

## Testing Checklist

- [ ] User can purchase tokens
- [ ] Position details display correctly
- [ ] Yield preview updates in real-time
- [ ] Rollover works when epoch complete
- [ ] Liquidation returns correct amount
- [ ] USDC balance updates after transactions
- [ ] Vault status displays correctly
- [ ] Error messages are user-friendly
- [ ] Loading states during transactions
- [ ] Transaction confirmation modals
