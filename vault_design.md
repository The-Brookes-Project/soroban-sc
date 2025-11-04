# Fractional Real Estate Tokenization Platform

**Platform:** Stellar Soroban Smart Contracts | **Model:** Debt-Based Investment | **Cycle:** 30-Day Rolling Windows

---

## 🎯 System Overview

Users invest in fractional real estate by purchasing property tokens with USDC stablecoins. After each 30-day epoch, they choose to **liquidate** (receive principal + yield, may be queued if vault depleted) or **rollover** (instantly continue earning for another 30 days). A shared liquidity vault with 10-20% buffer services all property liquidations via fair FIFO queuing when needed.

**Value Proposition:** Predictable 8-10% APY returns, low $100 minimum investment, flexible 30-day exit windows, rollovers incentivized with instant processing and maintained yield rates, transparent on-chain execution with controlled liquidity management.

---

## 🏗️ Core Architecture

### Smart Contracts (2-Tier Design)

```
┌─────────────────────────────────────────────────────────────────┐
│                    VaultContract (Singleton)                     │
│  • Manages $10M+ shared liquidity pool (USDC)                   │
│  • Maintains 10-20% liquidity buffer for normal operations      │
│  • Services liquidation requests from all properties             │
│  • Admin-controlled funding & authorization                      │
│  • QUEUES liquidations during low liquidity (controlled mode)   │
│  • Processes queue FIFO when liquidity replenished              │
└────────────────────────┬────────────────────────────────────────┘
                         │ authorizes & services
                         │
        ┌────────────────┼────────────────┬────────────────┐
        │                │                │                │
        ▼                ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Property A   │  │ Property B   │  │ Property C   │  │ Property N   │
│ Downtown     │  │ Suburban     │  │ Industrial   │  │ Future       │
│ Office       │  │ Retail       │  │ Warehouse    │  │ Properties   │
│              │  │              │  │              │  │              │
│ ROI: 8% APY  │  │ ROI: 10% APY │  │ ROI: 6% APY  │  │ Configurable │
│ Price: $100  │  │ Price: $50   │  │ Price: $200  │  │ Per Property │
│ Users: 1,000 │  │ Users: 500   │  │ Users: 300   │  │              │
└──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘
```

---

## ⚙️ 30-Day Epoch Mechanism

| Timeline | User Action | Smart Contract Logic | Result |
|----------|-------------|----------------------|--------|
| **Day 0** | Purchase 100 tokens @ $100 = $10,000 | Transfer USDC, create UserPosition, start epoch timer | Position created, epoch begins |
| **Day 1-29** | Monitor position, view accrued yield | Read-only queries, no state changes | Dashboard shows progress |
| **Day 30** | **Decision Time** | `can_take_action()` returns true | Two options available |
| **Option A: Liquidate** | Call `liquidate_position()` | Calculate yield ($66.67), request from vault queue, transfer $10,066.67 USDC to user when available | Position closed, funds received |
| **Option B: Rollover** | Call `rollover_position()` | **Maintain same yield rate**, reset epoch timer to Day 0, track total yield earned | New 30-day epoch starts (non-compounding) |

**Yield Calculation:** `Monthly Yield = Principal × (Annual_Rate / 12) / 10,000` → Example: $10,000 × (800 / 12) / 10,000 = **$66.67/month**

**Important:** Rollovers maintain the same yield per period (non-compounding). After 3 rollovers, total yield = 3 × $66.67 = $200. Liquidating and re-buying would compound but incurs **lower yield rates or fees** to incentivize rollovers.

---

## 💡 Key Features

### ✅ What Makes This Unique

| Feature | Description | Benefit |
|---------|-------------|---------|
| **30-Day Rolling Windows** | Fixed 2,592,000 second epochs with liquidation/rollover choice | Predictable timing, flexible exits |
| **Non-Compounding Rollovers** | Rollovers maintain same yield rate; liquidate+rebuy offers lower rates/fees | Incentivizes holding, predictable returns |
| **Controlled Liquidity Mode** | Vault queues liquidations when depleted, processes FIFO when replenished | Fair processing, maintains solvency |
| **Liquidity Buffer (10-20%)** | Reserve maintained for normal liquidations without new buyers | Smooth operations, reduced queuing |
| **Shared Liquidity Vault** | Single pool services all properties | Efficient capital use, lower reserve requirements |
| **Configurable ROI** | Each property sets own rate (6-10% APY) | Match risk/return profiles |
| **Fixed Property Values** | Debt model with fixed appraisal | Predictable returns, no price volatility |
| **Extremely Low Costs** | Soroban gas: ~$0.003/year vs Ethereum ~$500/year | 100-1000x cheaper transactions |

### 🔒 Security & Safety

- **No Reentrancy:** Soroban prevents by design
- **Overflow Protection:** All arithmetic uses `checked_add/sub/mul/div`
- **Authorization:** `require_auth()` on every state change
- **Emergency Pause:** Admin can halt vault in crisis
- **MultiSig Admin:** 3-of-5 Gnosis Safe for production
- **Audit-Ready:** Complete security checklist & best practices

---

## 💧 Vault Liquidity Modes

### Normal Mode (Available Liquidity > Buffer)
- **Buffer:** 10-20% of total capacity maintained
- **Liquidations:** Processed instantly
- **New purchases:** Fund the buffer for other liquidations
- **Status:** Green - System operating normally

### Controlled Liquidity Mode (Available < Buffer)
- **Trigger:** Available liquidity drops below buffer threshold
- **Action:** Pause new liquidations, queue pending requests
- **Queue:** FIFO (First In, First Out) processing
- **Resume:** When liquidity replenished via new purchases or admin funding
- **Status:** Yellow - Fair queuing active

### Critical Mode (Available ≈ 0)
- **Action:** Admin alerted, emergency funding required
- **Queue:** All liquidations paused until resolution
- **Communication:** Users notified of expected wait time
- **Status:** Red - Requires immediate admin intervention

**Liquidity Flow:**
```
New Buyers → USDC → Vault → 90% Available + 10% Buffer → Liquidation Requests
                                                              ↓
                                              Instant (Normal) or Queued (Controlled)
```

---

## 📊 Technical Specifications

### Smart Contract Stack
| Component | Technology | Purpose |
|-----------|-----------|---------|
| **Blockchain** | Stellar Mainnet | Layer 1 with 5s finality, 200+ TPS |
| **Smart Contracts** | Soroban (Rust → WASM) | Secure, efficient execution |
| **Token Standard** | Soroban Token Interface | USDC stablecoin for investments |
| **Storage** | Instance (global) + Persistent (per-user) | Optimized cost & access patterns |

### Key Data Structures
```rust
VaultConfig { 
    stablecoin: Address, 
    total_capacity: i128, 
    available: i128, 
    buffer_percentage: u32,        // 10-20% liquidity buffer
    controlled_mode: bool,          // True when queuing liquidations
    admin: Address 
}

LiquidationRequest {
    property: Address,
    user: Address, 
    amount: i128,
    timestamp: u64,
    request_id: u64                 // For FIFO processing
}

PropertyMetadata { 
    name: String, 
    roi_bps: u32,                   // Lower for liquidate+rebuy
    rollover_incentive_bps: u32,    // Additional yield for rollovers
    price: i128, 
    vault: Address 
}

UserPosition { 
    tokens: i128, 
    investment: i128,               // Fixed principal (non-compounding)
    epoch_start: u64, 
    epoch_count: u32,               // Track rollover history
    yield_earned: i128 
}
```
---

## 🚀 User Journey

```
1. CONNECT WALLET → 2. BROWSE PROPERTIES → 3. PURCHASE TOKENS → 4. WAIT 30 DAYS
   (Freighter)        (View ROI, pricing)     (Transfer USDC)      (Monitor yield)
                                                    ↓
5. EPOCH COMPLETE → 6a. LIQUIDATE ────────→ Request queued if low liquidity
   (Day 30)            (Exit investment)     Processed FIFO when vault replenished
                                             → Receive Principal + Yield → DONE
                                                ($10,066.67 USDC)
                    └→ 6b. ROLLOVER ───────→ Instant processing, maintain yield
                            (Keep earning)       (May earn bonus for loyalty)
                            No fees/delays       → Back to Step 4 (New epoch)
```

**Rollover Advantage:** Instant processing, maintain yield rate, potential loyalty bonus  
**Liquidate Trade-off:** May be queued during low liquidity, or face lower rates if re-buying immediately  
**Example:** $10,000 @ 8% APY → $66.67/month → After 3 rollovers = $10,200 total (non-compounded)

---

## ⚡ Quick Facts

| | |
|---|---|
| **Investment Minimum** | $100 (1 token @ $100) |
| **Epoch Duration** | Exactly 30 days (2,592,000 seconds) |
| **ROI Range** | 6-10% APY (configurable per property) |
| **Rollover Behavior** | Non-compounding; maintains same yield rate |
| **Liquidation Processing** | Instant (normal mode) or Queued (low liquidity) |
| **Liquidity Buffer** | 10-20% maintained for smooth operations |
| **Liquidate+Rebuy** | Lower yield or fees to incentivize rollovers |
| **Gas Cost (1 year)** | ~$0.003 total (purchase + 12 rollovers + liquidate) |
| **Finality** | 5 seconds (Stellar consensus) |
| **Token Lock (MVP)** | Yes (no secondary trading initially) |
| **Secondary Trading (Future)** | Phase 2 feature |

---
