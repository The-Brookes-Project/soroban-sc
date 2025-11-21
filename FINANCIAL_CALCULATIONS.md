# VerseToken Platform: Financial Calculations & Formulas

**Document Purpose:** This document details all financial calculations, formulas, and logic used in the VerseToken real estate tokenization platform for financial validation and audit purposes.

**Target Audience:** Financial analysts, accountants, and auditors (non-technical)

**Last Updated:** November 21, 2025

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Financial Calculations](#core-financial-calculations)
3. [Vault Liquidity Management](#vault-liquidity-management)
4. [Test Cases & Validation](#test-cases--validation)
5. [Risk Management Mechanisms](#risk-management-mechanisms)

---

## System Overview

### Architecture Components

The platform consists of three main financial components:

1. **Property Contracts** - Individual tokenized properties that calculate and accrue yield
2. **Liquidity Vault** - Centralized USDC reserve for processing liquidations
3. **KYC Contract** - Compliance verification (not covered in this document)

### Token Economics

- **Base Token:** USDC (stablecoin, 7 decimals on Stellar)
- **Property Tokens:** Fractional ownership tokens representing shares in specific properties
- **Yield Accrual:** Monthly epochs (30 days)

---

## Core Financial Calculations

### 1. Token Purchase Cost

When a user purchases property tokens, the cost is calculated as:

```
Cost (USDC) = (Token_Amount × Token_Price) ÷ 10^decimals
```

**Parameters:**

- `Token_Amount`: Number of tokens purchased (scaled by decimals)
- `Token_Price`: Price per token in USDC (scaled by decimals)
- `decimals`: Token decimal precision (typically 7 for Stellar assets)

**Example:**

```
User wants to buy 100 tokens
Token price = $10.00 per token
Decimals = 7

Token_Amount = 100 × 10^7 = 1,000,000,000
Token_Price = 10 × 10^7 = 100,000,000

Cost = (1,000,000,000 × 100,000,000) ÷ 10^7
     = 10,000,000,000,000,000 ÷ 10,000,000
     = 1,000,000,000 (scaled units)
     = $1,000.00 USDC
```

### 2. Yield Calculations

The platform uses a multi-tier yield system with three components:

#### 2.1 Base Yield

Base yield is the core return on investment, calculated monthly from an annual rate:

```
Monthly_Rate_BPS = Annual_Rate_BPS ÷ 12
Base_Yield = (Current_Principal × Monthly_Rate_BPS) ÷ 10,000
```

**Parameters:**

- `Annual_Rate_BPS`: Annual percentage yield in basis points (e.g., 800 = 8%)
- `Current_Principal`: User's current investment principal
- `10,000`: Basis point conversion factor (1 bps = 0.01%)

**Example - 8% APY:**

```
Annual_Rate_BPS = 800 (8% APY)
Principal = $10,000
Monthly_Rate_BPS = 800 ÷ 12 = 66.67 bps

Base_Yield = ($10,000 × 66.67) ÷ 10,000
           = $66.67 per month
```

**Validation (Test Case):**
From integration test line 66: User invests $10,000 at 8% APY for 30 days

- Expected yield: $66.67
- Total after 30 days: $10,066.67 ✓

#### 2.2 Compounding Bonus

Users who enable compounding receive an additional bonus yield:

```
Compounding_Bonus_Monthly = Compounding_Bonus_BPS ÷ 12
Compounding_Bonus = (Current_Principal × Compounding_Bonus_Monthly) ÷ 10,000
```

**Default Configuration:**

- `Compounding_Bonus_BPS`: 200 basis points (2% annual bonus)

**Example:**

```
Principal = $10,000
Compounding_Bonus_BPS = 200
Compounding_Bonus_Monthly = 200 ÷ 12 = 16.67 bps

Compounding_Bonus = ($10,000 × 16.67) ÷ 10,000
                  = $16.67 per month
```

**Note:** When compounding is enabled, yield is automatically added to principal each epoch:

```
New_Principal = Current_Principal + Base_Yield + Compounding_Bonus + Loyalty_Bonus
```

#### 2.3 Loyalty Bonus

Users who repeatedly roll over their positions earn increasing loyalty bonuses:

```
Loyalty_Tier = min(Consecutive_Rollovers, 4)
Monthly_Loyalty_Rate = (Loyalty_Tier × Loyalty_Bonus_BPS) ÷ 12
Loyalty_Bonus = (Current_Principal × Monthly_Loyalty_Rate) ÷ 10,000
```

**Parameters:**

- `Loyalty_Bonus_BPS`: 25 basis points per tier (default)
- `Maximum Tiers`: 4 (caps at tier 4 regardless of rollovers)

**Tier Structure:**

| Tier | Consecutive Rollovers | Annual Bonus | Monthly Rate (BPS) |
| ---- | --------------------- | ------------ | ------------------ |
| 0    | 0                     | 0%           | 0                  |
| 1    | 1                     | 0.25%        | 2.08               |
| 2    | 2                     | 0.50%        | 4.17               |
| 3    | 3                     | 0.75%        | 6.25               |
| 4    | 4+                    | 1.00%        | 8.33               |

**Example - Tier 4:**

```
Principal = $1,000,000
Loyalty_Tier = 4
Loyalty_Bonus_BPS = 25
Monthly_Rate = (4 × 25) ÷ 12 = 8.33 bps

Loyalty_Bonus = ($1,000,000 × 8.33) ÷ 10,000
              = $833.33 per month
```

**Validation (Test Case):**
From integration test line 287-294: Loyalty bonus progression

- Tier 0: Base rate only
- Tier 4: Maximum loyalty bonus applied ✓

#### 2.4 Total Yield Formula

The complete yield calculation combines all components:

```
Total_Yield = Base_Yield + Compounding_Bonus + Loyalty_Bonus

where:
  Base_Yield = (Principal × Annual_Rate ÷ 12) ÷ 10,000
  Compounding_Bonus = (Principal × Bonus_Rate ÷ 12) ÷ 10,000  [if enabled]
  Loyalty_Bonus = (Principal × Tier × Loyalty_Rate ÷ 12) ÷ 10,000
```

**Complete Example:**

```
Principal = $1,000,000
Annual_Rate = 800 bps (8%)
Compounding = enabled (200 bps bonus)
Loyalty_Tier = 4

Base_Yield = ($1,000,000 × 800 ÷ 12) ÷ 10,000
           = $6,666.67

Compounding_Bonus = ($1,000,000 × 200 ÷ 12) ÷ 10,000
                  = $1,666.67

Loyalty_Bonus = ($1,000,000 × 4 × 25 ÷ 12) ÷ 10,000
              = $833.33

Total_Yield = $6,666.67 + $1,666.67 + $833.33
            = $9,166.67 per month
```

**Effective APY Calculation:**

```
Effective_Annual_Yield = ($9,166.67 × 12) ÷ $1,000,000
                       = $110,000 ÷ $1,000,000
                       = 11% APY
```

### 3. Liquidation Payout

When a user liquidates their position, they receive:

```
Total_Payout = Current_Principal + Final_Epoch_Yield
```

**Important Notes:**

- Users must complete the current epoch before liquidating (30 days minimum)
- Final epoch yield is calculated using the formulas above
- If user had compounding enabled, `Current_Principal` includes all previously compounded yields
- User loses loyalty tier benefits upon liquidation

**Example:**

```
User starts with $10,000
8% base APY + 2% compounding bonus + Tier 2 loyalty (0.5%)
After 3 months of compounding:

Month 1:
  Principal = $10,000.00
  Yield = $87.50 (base $66.67 + comp $16.67 + loyalty $4.17)
  New Principal = $10,087.50

Month 2:
  Principal = $10,087.50
  Yield = $88.27
  New Principal = $10,175.77

Month 3:
  Principal = $10,175.77
  Yield = $89.04
  Total Payout = $10,264.81
```

---

## Vault Liquidity Management

### 4. Buffer Calculation

The vault maintains a liquidity buffer to prevent insolvency:

```
Buffer_Amount = (Total_Capacity × Buffer_Percentage) ÷ 100
Available_For_Liquidation = Available_Liquidity - Buffer_Amount
```

**Parameters:**

- `Total_Capacity`: Total USDC deposited in vault
- `Buffer_Percentage`: Safety buffer (10-25%, default 15%)
- `Available_Liquidity`: Current USDC balance in vault

**Example:**

```
Total_Capacity = $10,000,000
Buffer_Percentage = 15%
Current_Available = $10,000,000

Buffer_Amount = ($10,000,000 × 15) ÷ 100
              = $1,500,000

Available_For_Liquidation = $10,000,000 - $1,500,000
                          = $8,500,000
```

**Validation (Test Case):**
From integration test line 404-407:

- Vault with $1M can process up to $850k
- Remaining $150k held as 15% buffer ✓

### 5. Instant Processing vs. Queuing Logic

When a liquidation is requested, the system determines processing method:

```
Required_Available = Buffer_Threshold + Liquidation_Amount

IF Available_Liquidity >= Required_Available AND NOT Controlled_Mode:
    Process Immediately
ELSE:
    Queue Request (Enter Controlled Mode)
```

**Example Scenario:**

```
Vault State:
- Total_Capacity = $1,000,000
- Available = $200,000
- Buffer = 15% = $150,000

User requests $100,000 liquidation:
Required = $150,000 + $100,000 = $250,000
Available = $200,000

Since $200,000 < $250,000:
→ Request is QUEUED
→ Controlled Mode ACTIVATED
```

**Validation (Test Case):**
From integration test line 213-234:

- Vault with $1M attempts $900k liquidation
- Would leave only $100k (10% < 15% buffer)
- Request is queued, not processed ✓

### 6. Queue Processing Order

Queued liquidations are processed in strict FIFO (First In, First Out) order:

```
FOR each request in queue (oldest first):
    Required = Buffer_Threshold + Request_Amount

    IF Available_Liquidity >= Required:
        Process Request
        Available_Liquidity -= Request_Amount
        Remove from Queue
    ELSE:
        Stop Processing (wait for more liquidity)
```

**Example:**

```
Queue State:
1. User A: $200,000
2. User B: $150,000
3. User C: $300,000

Available: $500,000
Buffer: $150,000

Processing User A:
  Required = $150,000 + $200,000 = $350,000
  Available = $500,000 ✓
  Process → Available now $300,000

Processing User B:
  Required = $150,000 + $150,000 = $300,000
  Available = $300,000 ✓
  Process → Available now $150,000

Processing User C:
  Required = $150,000 + $300,000 = $450,000
  Available = $150,000 ✗
  Queue (wait for funding)
```

**Validation (Test Case):**
From integration test line 337-386:

- Multiple users queue during low liquidity
- Admin adds liquidity
- Queue processes in order automatically ✓

### 7. Controlled Mode Activation/Deactivation

**Activation Triggers:**

```
IF Available_Liquidity < (Buffer_Threshold + Requested_Amount):
    Controlled_Mode = TRUE
```

**Deactivation Condition:**

```
IF Queue is Empty (head_index >= tail_index):
    Controlled_Mode = FALSE
```

### 8. Fulfillment Date Estimation

For queued liquidations, the system estimates processing time:

```
Expected_Monthly_Cash_Flow = Sum of all property monthly cash flows
Months_Needed = Total_Queue_Amount ÷ Expected_Monthly_Cash_Flow
Months_Capped = min(Months_Needed, 12)  // Cap at 1 year
Estimated_Fulfillment = Current_Time + (Months_Capped × 30 days)
```

**Example:**

```
Queue Amount: $1,000,000
Property A Monthly Cash Flow: $50,000
Property B Monthly Cash Flow: $30,000
Property C Monthly Cash Flow: $20,000

Total_Monthly_Cash_Flow = $100,000

Months_Needed = $1,000,000 ÷ $100,000 = 10 months
Estimated_Date = Today + 300 days
```

**Note:** If no cash flow data available, defaults to 90 days estimate.

### 9. Withdrawal Rules

Admin can withdraw excess liquidity, but must maintain:

```
Minimum_Required = Buffer_Amount + Queue_Obligations

Queue_Obligations = Sum of all pending liquidation amounts

Available_After_Withdrawal = Current_Available - Withdrawal_Amount

IF Available_After_Withdrawal < Minimum_Required:
    REJECT Withdrawal
```

**Example:**

```
Vault State:
- Total_Capacity = $5,000,000
- Available = $4,000,000
- Buffer (15%) = $750,000
- Queue_Obligations = $1,000,000

Minimum_Required = $750,000 + $1,000,000 = $1,750,000

Admin requests $2,500,000 withdrawal:
Available_After = $4,000,000 - $2,500,000 = $1,500,000

Since $1,500,000 < $1,750,000:
→ Withdrawal REJECTED
```

---

## Test Cases & Validation

### Test Case 1: Basic Property Lifecycle

**Scenario:** Users invest and liquidate from multiple properties

**Setup:**

- Vault funded with $10,000,000
- Properties A and B authorized
- 8% APY (Property A), 10% APY (Property B)

**Transactions:**

| User | Property | Investment | Duration | APY | Expected Yield | Total Payout |
| ---- | -------- | ---------- | -------- | --- | -------------- | ------------ |
| 1    | A        | $10,000    | 30 days  | 8%  | $66.67         | $10,066.67   |
| 2    | A        | $5,000     | 30 days  | 8%  | $33.33         | $5,033.33    |
| 3    | B        | $20,000    | 30 days  | 10% | $166.67        | $20,166.67   |

**Validation:**

```
Total Liquidated: $35,266.67
Remaining Vault: $10,000,000 - $35,266.67 = $9,964,733.33 ✓

Property A Stats:
- Total Liquidated: $15,100.00 ✓

Property B Stats:
- Total Liquidated: $20,166.67 ✓
```

**Source:** integration_test.rs lines 44-93

### Test Case 2: Queue Management with Limited Liquidity

**Scenario:** Low liquidity triggers controlled mode with queue processing

**Setup:**

- Vault funded with $500,000
- Buffer 15% = $75,000
- Maximum processable = $425,000

**Liquidation Requests:**

| Order | User | Property | Amount   | Status      | Reason                        |
| ----- | ---- | -------- | -------- | ----------- | ----------------------------- |
| 1     | A1   | A        | $200,000 | ✓ Processed | Under limit ($425k available) |
| 2     | A2   | A        | $150,000 | ⏳ Queued   | After #1: only $225k left     |
| 3     | B1   | B        | $300,000 | ⏳ Queued   | Exceeds remaining capacity    |
| 4     | C1   | C        | $100,000 | ⏳ Queued   | Vault in controlled mode      |

**After Admin Funds Additional $1,000,000:**

| User | Status    | New Balance |
| ---- | --------- | ----------- |
| A1   | Completed | $200,000    |
| A2   | Processed | $150,000    |
| B1   | Processed | $300,000    |
| C1   | Processed | $100,000    |

**Validation:**

- Controlled mode activated when buffer threatened ✓
- FIFO processing order maintained ✓
- Controlled mode deactivated when queue cleared ✓

**Source:** integration_test.rs lines 95-148

### Test Case 3: Buffer Protection

**Scenario:** System prevents liquidations that would breach buffer requirements

**Setup:**

- Vault with $1,000,000
- Buffer 15% = $150,000
- Available for use = $850,000

**Test:**

```
User requests $900,000 liquidation

Check:
Remaining_After = $1,000,000 - $900,000 = $100,000
Buffer_Required = $150,000

Since $100,000 < $150,000:
→ Request QUEUED (not processed)
→ Controlled Mode ACTIVATED
```

**Validation:**

- User balance remains $0 (queued) ✓
- Vault maintains full $1M balance ✓
- Queue status shows $900k pending ✓

**Source:** integration_test.rs lines 204-234

### Test Case 4: Compounding Math

**Scenario:** Verify compounding bonus calculations and principal growth

**Setup:**

- Base investment: $1,000,000
- Base APY: 8% (800 bps)
- Compounding bonus: 2% (200 bps)
- Duration: 3 months

**Non-Compounding User:**

```
Month 1: $1,000,000 × (800 ÷ 12) ÷ 10,000 = $6,667
Month 2: $1,000,000 × (800 ÷ 12) ÷ 10,000 = $6,667
Month 3: $1,000,000 × (800 ÷ 12) ÷ 10,000 = $6,667
Total: $1,020,000
```

**Compounding User:**

```
Month 1:
  Principal = $1,000,000
  Base = $6,667
  Bonus = $1,667
  Total Yield = $8,334
  New Principal = $1,008,334

Month 2:
  Principal = $1,008,334
  Base = $6,722
  Bonus = $1,681
  Total Yield = $8,403
  New Principal = $1,016,737

Month 3:
  Principal = $1,016,737
  Base = $6,778
  Bonus = $1,695
  Total Yield = $8,473
  New Principal = $1,025,210

Final: $1,025,210 (vs $1,020,000 non-compounding)
Difference: $5,210 extra from compounding
```

**Validation:**

- Compounding user receives higher returns ✓
- Principal grows each epoch for compounders ✓
- Non-compounders receive flat yield ✓

**Source:** integration_test.rs lines 237-267

### Test Case 5: Loyalty Tier Progression

**Scenario:** Verify loyalty bonus increases with consecutive rollovers

**Setup:**

- Principal: $1,000,000
- Base APY: 8%
- Loyalty bonus: 25 bps per tier

**Rollover Sequence:**

| Rollover | Tier | Monthly Loyalty Rate | Loyalty Yield | Total Monthly Yield |
| -------- | ---- | -------------------- | ------------- | ------------------- |
| 0        | 0    | 0.00 bps             | $0            | $6,667              |
| 1        | 1    | 2.08 bps             | $208          | $6,875              |
| 2        | 2    | 4.17 bps             | $417          | $7,084              |
| 3        | 3    | 6.25 bps             | $625          | $7,292              |
| 4        | 4    | 8.33 bps             | $833          | $7,500              |
| 5        | 4    | 8.33 bps             | $833          | $7,500              |

**Note:** Tier caps at 4, so rollover 5+ all receive tier 4 benefits.

**Validation:**

- Tier increases with each rollover (max 4) ✓
- Loyalty bonus compounds with principal if compounding enabled ✓
- Tier resets to 0 upon liquidation ✓

**Source:** integration_test.rs lines 270-295

### Test Case 6: Buffer Adjustment Impact

**Scenario:** Changing buffer percentage affects liquidation processing

**Setup:**

- Vault: $1,000,000
- Initial buffer: 15% ($150,000)
- Available for liquidation: $850,000

**Test:**

```
User 1 liquidates $850,000
Remaining: $150,000

Admin increases buffer to 20%
New Buffer_Required = $1,000,000 × 20% = $200,000
Current Available = $150,000

Since $150,000 < $200,000:
→ Controlled Mode ACTIVATED
→ New liquidation requests will queue
```

**Validation:**

- Buffer adjustment takes effect immediately ✓
- May trigger controlled mode if current balance insufficient ✓
- Protects vault from over-liquidation ✓

**Source:** integration_test.rs lines 390-425

### Test Case 7: Statistics Tracking

**Scenario:** Verify accurate tracking of property liquidation statistics

**Setup:**

- Property authorized in vault
- Multiple liquidations processed

**Transactions:**

| User | Amount   | Running Total |
| ---- | -------- | ------------- |
| A    | $100,000 | $100,000      |
| B    | $250,000 | $350,000      |
| C    | $175,000 | $525,000      |
| D    | $500,000 | $1,025,000    |

**Validation:**

```
Property Stats:
- total_liquidated: $1,025,000 ✓
- last_liquidation: timestamp of User D ✓
```

**Source:** integration_test.rs lines 428-463

---

## Risk Management Mechanisms

### 1. Buffer System

**Purpose:** Maintain minimum liquidity reserve to handle unexpected redemptions

**Mechanism:**

- Configurable percentage (10-25%)
- Calculated on total vault capacity
- Enforced before every liquidation
- Prevents insolvency

**Financial Impact:**

```
If Buffer = 15% and Vault = $10M:
  Reserved = $1.5M (unavailable for liquidation)
  Working Capital = $8.5M (available for operations)
```

### 2. Controlled Mode

**Purpose:** Protect vault during liquidity constraints

**Triggers:**

- Available liquidity falls below buffer threshold
- Large liquidation request exceeds available capacity

**Effects:**

- New liquidations automatically queue
- FIFO processing when liquidity restored
- Transparent to users (estimated fulfillment dates provided)

**Exit Condition:**

- Queue fully processed
- Normal mode resumes automatically

### 3. Authorization System

**Purpose:** Prevent unauthorized property contracts from draining vault

**Mechanism:**

- Admin must explicitly authorize each property contract
- Only authorized contracts can request liquidations
- Per-property statistics tracked
- Protects against malicious contracts

### 4. Emergency Pause

**Purpose:** Admin can halt all liquidations during crisis

**Mechanism:**

- Immediate effect
- Blocks all new liquidations
- Existing queue preserved
- Requires explicit unpause to resume

**Use Cases:**

- Smart contract vulnerability discovered
- Suspicious activity detected
- Regulatory hold
- System maintenance

### 5. Epoch Lock-In

**Purpose:** Ensure users complete full investment periods

**Mechanism:**

- Minimum 30-day holding period
- Users cannot liquidate mid-epoch
- Prevents rapid in/out trading
- Stabilizes liquidity planning

**Grace Period:**

- 24-hour window after epoch ends
- User can rollover or liquidate
- After grace period: admin can force rollover
- Protects passive users from missed opportunities

### 6. Overflow Protection

**Purpose:** Prevent arithmetic overflow errors in financial calculations

**Mechanism:**

- All arithmetic operations use checked math
- Overflow triggers transaction failure (not silent corruption)
- Protects against extreme edge cases

**Example:**

```rust
// Safe addition with overflow check
available = available.checked_add(amount).expect("Overflow in available");

// vs unsafe (not used):
available = available + amount;  // Could silently overflow
```

---

## Appendix: Constants & Configuration

### Time Constants

| Constant       | Value    | Seconds   | Description              |
| -------------- | -------- | --------- | ------------------------ |
| EPOCH_DURATION | 30 days  | 2,592,000 | Investment period length |
| GRACE_PERIOD   | 24 hours | 86,400    | Post-epoch action window |

### Rate Limits

| Parameter                | Minimum | Default | Maximum | Unit |
| ------------------------ | ------- | ------- | ------- | ---- |
| Buffer Percentage        | 10      | 15      | 25      | %    |
| Annual ROI (Base)        | 0       | 800     | 2,000   | bps  |
| Compounding Bonus        | 0       | 200     | -       | bps  |
| Loyalty Bonus (per tier) | 0       | 25      | -       | bps  |
| Max Loyalty Tier         | -       | 4       | 4       | tier |

### Decimal Precision

- **USDC:** 7 decimals (Stellar standard)
- **Property Tokens:** Configurable, max 7 decimals
- **Basis Points:** 10,000 = 100% (0.01% precision)

---

## Glossary

**Annual Percentage Yield (APY):** The effective annual rate of return taking into account compounding.

**Basis Point (bps):** 1/100th of a percent (1 bps = 0.01%). Used for precise financial calculations.

**Buffer:** Reserved liquidity that cannot be used for liquidations, maintained as safety margin.

**Compounding:** Automatically reinvesting yield into principal for exponential growth.

**Controlled Mode:** Vault state where liquidations are queued due to insufficient immediate liquidity.

**Epoch:** Fixed 30-day investment period. Users can only liquidate or rollover at epoch boundaries.

**FIFO (First In, First Out):** Queue processing method where earliest requests are fulfilled first.

**Grace Period:** 24-hour window after epoch ends where user can take action before admin intervention.

**Liquidation:** Converting property position back to USDC, including principal and yield.

**Loyalty Tier:** Level (0-4) earned through consecutive rollovers, providing additional yield bonuses.

**Principal:** The current invested amount, may include compounded yields if compounding enabled.

**Rollover:** Extending position for another epoch, capturing earned yield and advancing loyalty tier.

**Yield:** The return on investment earned during an epoch, paid in USDC.

---

## Document Revision History

| Date       | Version | Changes                                                |
| ---------- | ------- | ------------------------------------------------------ |
| 2025-11-21 | 1.0     | Initial documentation with all formulas and test cases |

---

## Verification Checklist

For financial auditors validating this system:

- [ ] Verify all yield formulas calculate correctly (see Test Cases section)
- [ ] Confirm buffer calculations maintain required reserves
- [ ] Validate queue processing maintains FIFO order
- [ ] Check compounding math produces accurate principal growth
- [ ] Verify loyalty bonuses scale correctly with tiers
- [ ] Confirm overflow protection prevents arithmetic errors
- [ ] Validate liquidation payouts include all earned yield
- [ ] Check emergency pause prevents unauthorized fund movement
- [ ] Verify property authorization prevents unauthorized access
- [ ] Confirm statistics tracking accurately reflects all transactions

---

**For Questions or Clarifications:** Please contact the development team with reference to specific test cases or formula sections in this document.
