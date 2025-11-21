#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Symbol, symbol_short};

// Constants
const EPOCH_DURATION: u64 = 2_592_000;  // 30 days in seconds
const GRACE_PERIOD: u64 = 86_400;       // 24 hours in seconds
const MAX_LOYALTY_TIER: u32 = 4;

// Storage keys
const METADATA_KEY: Symbol = symbol_short!("METADATA");
const ROI_CONFIG_KEY: Symbol = symbol_short!("ROI_CFG");
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const TOTAL_ACTIVE_KEY: Symbol = symbol_short!("TOTAL");

// Error codes
pub const ERR_ALREADY_INIT: u32 = 1;
pub const ERR_NOT_ADMIN: u32 = 2;
pub const ERR_INVALID_AMOUNT: u32 = 3;
pub const ERR_POSITION_EXISTS: u32 = 4;
pub const ERR_NO_POSITION: u32 = 5;
pub const ERR_EPOCH_NOT_COMPLETE: u32 = 6;
pub const ERR_GRACE_PERIOD_NOT_PASSED: u32 = 7;

// Property metadata
#[contracttype]
#[derive(Clone, Debug)]
pub struct PropertyMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub total_supply: i128,
    pub token_price: i128,              // Price per token in USDC
    pub vault_address: Address,          // Shared vault
    pub kyc_address: Address,            // Shared KYC contract
    pub stablecoin_address: Address,     // USDC
}

// ROI configuration
#[contracttype]
#[derive(Clone, Debug)]
pub struct RoiConfig {
    pub annual_rate_bps: u32,            // Base APY (e.g., 800 = 8%)
    pub compounding_bonus_bps: u32,      // Bonus for compounding (e.g., 200 = +2%)
    pub loyalty_bonus_bps: u32,          // Per-tier bonus (25 bps)
    pub cash_flow_monthly: i128,         // Expected monthly cash flow
}

// User position
#[contracttype]
#[derive(Clone, Debug)]
pub struct UserPosition {
    pub tokens: i128,
    pub initial_investment: i128,
    pub current_principal: i128,
    pub compounding_enabled: bool,
    pub epoch_start: u64,
    pub consecutive_rollovers: u32,
    pub total_yield_earned: i128,
    pub loyalty_tier: u32,               // 0-4
}

// Yield preview for users
#[contracttype]
#[derive(Clone, Debug)]
pub struct YieldPreview {
    pub base_yield: i128,
    pub compounding_bonus: i128,
    pub loyalty_bonus: i128,
    pub total_yield: i128,
    pub days_elapsed: u32,
    pub days_remaining: u32,
}

// Storage key types
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    UserPosition(Address),
}

// Event types
#[contracttype]
#[derive(Clone, Debug)]
pub enum PropertyEvent {
    Initialized(Address),
    TokensPurchased(Address, i128, i128, bool),  // buyer, tokens, cost, compounding
    PositionRolledOver(Address, u32, i128, u32, i128, bool),  // user, rollovers, yield, tier, principal, admin_triggered
    PositionLiquidated(Address, i128, i128, i128, u32),  // user, principal, yield, total, rollovers
    RoiConfigUpdated(Address, u32, u32, u32),  // admin, annual, comp_bonus, loyalty_bonus
}

#[contract]
pub struct PropertyContract;

#[contractimpl]
impl PropertyContract {
    /// Initialize property contract
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
        token_price: i128,
        vault_address: Address,
        kyc_address: Address,
        stablecoin_address: Address,
    ) {
        admin.require_auth();

        // Check if already initialized
        if env.storage().instance().has(&METADATA_KEY) {
            panic!("Property contract already initialized");
        }

        // Validate parameters
        if total_supply <= 0 {
            panic!("Total supply must be positive");
        }
        if decimals > 7 {
            panic!("Decimals cannot exceed 7");
        }
        if token_price <= 0 {
            panic!("Token price must be positive");
        }

        // Create and store metadata
        let metadata = PropertyMetadata {
            name,
            symbol,
            decimals,
            total_supply,
            token_price,
            vault_address,
            kyc_address,
            stablecoin_address,
        };
        env.storage().instance().set(&METADATA_KEY, &metadata);

        // Create and store default ROI config (admin can update later)
        let roi_config = RoiConfig {
            annual_rate_bps: 800,  // Default 8% APY
            compounding_bonus_bps: 200,  // Default +2% bonus
            loyalty_bonus_bps: 25,  // Default 25 bps per tier
            cash_flow_monthly: 0,  // Admin sets this later
        };
        env.storage().instance().set(&ROI_CONFIG_KEY, &roi_config);

        // Store admin
        env.storage().instance().set(&ADMIN_KEY, &admin);

        // Initialize total active tokens
        env.storage().instance().set(&TOTAL_ACTIVE_KEY, &0i128);

        // Emit event
        env.events().publish(
            (symbol_short!("init"),),
            PropertyEvent::Initialized(admin),
        );
    }

    /// Admin updates ROI configuration
    pub fn update_roi_config(
        env: Env,
        admin: Address,
        annual_rate_bps: u32,
        compounding_bonus_bps: u32,
        loyalty_bonus_bps: u32,
        cash_flow_monthly: i128,
    ) {
        admin.require_auth();

        // Verify caller is admin
        let stored_admin: Address = env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Property contract not initialized");
        
        if admin != stored_admin {
            panic!("Not admin");
        }

        // Validate
        if annual_rate_bps == 0 || annual_rate_bps > 2000 {
            panic!("Annual rate must be between 0 and 2000 bps");
        }

        // Update ROI config
        let roi_config = RoiConfig {
            annual_rate_bps,
            compounding_bonus_bps,
            loyalty_bonus_bps,
            cash_flow_monthly,
        };
        env.storage().instance().set(&ROI_CONFIG_KEY, &roi_config);

        // Emit event
        env.events().publish(
            (symbol_short!("roi_upd"),),
            PropertyEvent::RoiConfigUpdated(admin, annual_rate_bps, compounding_bonus_bps, loyalty_bonus_bps),
        );
    }

    /// User purchases property tokens
    pub fn purchase_tokens(
        env: Env,
        buyer: Address,
        token_amount: i128,
        enable_compounding: bool,
    ) {
        buyer.require_auth();

        // Load metadata
        let metadata = Self::get_metadata(&env);

        // Check if user already has a position
        if env.storage().persistent().has(&DataKey::UserPosition(buyer.clone())) {
            panic!("Position already exists");
        }

        // Validate amount
        if token_amount <= 0 {
            panic!("Invalid token amount");
        }

        // Check KYC/compliance via KYC contract
        let kyc_client = KycContractClient::new(&env, &metadata.kyc_address);
        kyc_client.check_compliance(&buyer);

        // Calculate cost
        let cost = token_amount.checked_mul(metadata.token_price)
            .expect("Overflow in cost calculation");

        // Transfer USDC from buyer to contract
        let token_client = token::Client::new(&env, &metadata.stablecoin_address);
        
        // Verify buyer has sufficient balance
        let buyer_balance = token_client.balance(&buyer);
        if buyer_balance < cost {
            panic!("Insufficient USDC balance");
        }

        token_client.transfer(&buyer, &env.current_contract_address(), &cost);

        // Create user position
        let position = UserPosition {
            tokens: token_amount,
            initial_investment: cost,
            current_principal: cost,
            compounding_enabled: enable_compounding,
            epoch_start: env.ledger().timestamp(),
            consecutive_rollovers: 0,
            total_yield_earned: 0,
            loyalty_tier: 0,
        };

        // Store position
        env.storage().persistent().set(&DataKey::UserPosition(buyer.clone()), &position);

        // Update total active tokens
        let mut total_active: i128 = env.storage().instance().get(&TOTAL_ACTIVE_KEY).unwrap_or(0);
        total_active = total_active.checked_add(token_amount)
            .expect("Overflow in total active");
        env.storage().instance().set(&TOTAL_ACTIVE_KEY, &total_active);

        // Emit event
        env.events().publish(
            (symbol_short!("purchase"),),
            PropertyEvent::TokensPurchased(buyer, token_amount, cost, enable_compounding),
        );
    }

    /// User rolls over position for another epoch
    pub fn rollover_position(
        env: Env,
        user: Address,
    ) {
        user.require_auth();

        // Load position
        let mut position: UserPosition = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user.clone()))
            .expect("No position found");

        // Load ROI config
        let roi_config = Self::get_roi_config(&env);

        // Check epoch is complete
        let current_time = env.ledger().timestamp();
        let epoch_end = position.epoch_start.checked_add(EPOCH_DURATION)
            .expect("Overflow in epoch calculation");
        
        if current_time < epoch_end {
            panic!("Epoch not complete");
        }

        // Calculate yield
        let (base_yield, compounding_bonus, loyalty_bonus) = Self::calculate_yield(&position, &roi_config);
        let total_yield = base_yield.checked_add(compounding_bonus)
            .expect("Overflow")
            .checked_add(loyalty_bonus)
            .expect("Overflow");

        // Update position based on compounding preference
        if position.compounding_enabled {
            // Add yield to principal
            position.current_principal = position.current_principal.checked_add(total_yield)
                .expect("Overflow in principal");
        }

        // Track total yield earned
        position.total_yield_earned = position.total_yield_earned.checked_add(total_yield)
            .expect("Overflow in total yield");

        // Increment loyalty tier
        position.consecutive_rollovers = position.consecutive_rollovers.checked_add(1)
            .expect("Overflow in rollovers");
        position.loyalty_tier = if position.consecutive_rollovers >= MAX_LOYALTY_TIER {
            MAX_LOYALTY_TIER
        } else {
            position.consecutive_rollovers
        };

        // Reset epoch timer
        position.epoch_start = current_time;

        // Store updated position
        env.storage().persistent().set(&DataKey::UserPosition(user.clone()), &position);

        // Emit event
        env.events().publish(
            (symbol_short!("rollover"),),
            PropertyEvent::PositionRolledOver(
                user,
                position.consecutive_rollovers,
                total_yield,
                position.loyalty_tier,
                position.current_principal,
                false,  // not admin triggered
            ),
        );
    }

    /// Admin rolls over position after grace period
    pub fn admin_rollover_position(
        env: Env,
        admin: Address,
        user: Address,
    ) {
        admin.require_auth();

        // Verify caller is admin
        let stored_admin: Address = env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Property contract not initialized");
        
        if admin != stored_admin {
            panic!("Not admin");
        }

        // Load position
        let mut position: UserPosition = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user.clone()))
            .expect("No position found");

        // Load ROI config
        let roi_config = Self::get_roi_config(&env);

        // Check grace period has passed
        let current_time = env.ledger().timestamp();
        let grace_period_end = position.epoch_start
            .checked_add(EPOCH_DURATION)
            .expect("Overflow")
            .checked_add(GRACE_PERIOD)
            .expect("Overflow");
        
        if current_time < grace_period_end {
            panic!("Grace period not passed");
        }

        // Calculate yield
        let (base_yield, compounding_bonus, loyalty_bonus) = Self::calculate_yield(&position, &roi_config);
        let total_yield = base_yield.checked_add(compounding_bonus)
            .expect("Overflow")
            .checked_add(loyalty_bonus)
            .expect("Overflow");

        // Update position based on compounding preference
        if position.compounding_enabled {
            position.current_principal = position.current_principal.checked_add(total_yield)
                .expect("Overflow in principal");
        }

        position.total_yield_earned = position.total_yield_earned.checked_add(total_yield)
            .expect("Overflow in total yield");

        // Increment loyalty tier
        position.consecutive_rollovers = position.consecutive_rollovers.checked_add(1)
            .expect("Overflow in rollovers");
        position.loyalty_tier = if position.consecutive_rollovers >= MAX_LOYALTY_TIER {
            MAX_LOYALTY_TIER
        } else {
            position.consecutive_rollovers
        };

        // Reset epoch timer
        position.epoch_start = current_time;

        // Store updated position
        env.storage().persistent().set(&DataKey::UserPosition(user.clone()), &position);

        // Emit event with admin flag
        env.events().publish(
            (symbol_short!("rollover"),),
            PropertyEvent::PositionRolledOver(
                user,
                position.consecutive_rollovers,
                total_yield,
                position.loyalty_tier,
                position.current_principal,
                true,  // admin triggered
            ),
        );
    }

    /// User liquidates position
    pub fn liquidate_position(
        env: Env,
        user: Address,
    ) {
        user.require_auth();

        // Load position
        let position: UserPosition = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user.clone()))
            .expect("No position found");

        // Load metadata and ROI config
        let metadata = Self::get_metadata(&env);
        let roi_config = Self::get_roi_config(&env);

        // Check epoch is complete
        let current_time = env.ledger().timestamp();
        let epoch_end = position.epoch_start.checked_add(EPOCH_DURATION)
            .expect("Overflow in epoch calculation");
        
        if current_time < epoch_end {
            panic!("Epoch not complete");
        }

        // Calculate final yield for this epoch
        let (base_yield, compounding_bonus, loyalty_bonus) = Self::calculate_yield(&position, &roi_config);
        let final_epoch_yield = base_yield.checked_add(compounding_bonus)
            .expect("Overflow")
            .checked_add(loyalty_bonus)
            .expect("Overflow");

        // Calculate total payout
        let total_payout = position.current_principal.checked_add(final_epoch_yield)
            .expect("Overflow in payout calculation");

        // Request liquidation from vault
        let vault_client = VaultContractClient::new(&env, &metadata.vault_address);
        vault_client.request_liquidation(
            &env.current_contract_address(),
            &user,
            &total_payout,
        );

        // Remove position from storage
        env.storage().persistent().remove(&DataKey::UserPosition(user.clone()));

        // Update total active tokens
        let mut total_active: i128 = env.storage().instance().get(&TOTAL_ACTIVE_KEY).unwrap_or(0);
        total_active = total_active.checked_sub(position.tokens)
            .expect("Underflow in total active");
        env.storage().instance().set(&TOTAL_ACTIVE_KEY, &total_active);

        // Emit event
        env.events().publish(
            (symbol_short!("liquidate"),),
            PropertyEvent::PositionLiquidated(
                user,
                position.current_principal,
                final_epoch_yield,
                total_payout,
                position.consecutive_rollovers,
            ),
        );
    }

    // View functions

    /// Get user's current position
    pub fn get_user_position(
        env: Env,
        user: Address,
    ) -> Option<UserPosition> {
        env.storage().persistent().get(&DataKey::UserPosition(user))
    }

    /// Preview yield for current epoch
    pub fn preview_yield(
        env: Env,
        user: Address,
    ) -> YieldPreview {
        // Load position
        let position: UserPosition = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user))
            .expect("No position found");

        // Load ROI config
        let roi_config = Self::get_roi_config(&env);

        // Calculate time in epoch
        let current_time = env.ledger().timestamp();
        let elapsed = current_time.checked_sub(position.epoch_start)
            .unwrap_or(0);
        let days_elapsed = (elapsed / 86_400) as u32;  // seconds per day
        let days_remaining = if days_elapsed >= 30 { 0 } else { 30 - days_elapsed };

        // Calculate yield components
        let (base_yield, compounding_bonus, loyalty_bonus) = Self::calculate_yield(&position, &roi_config);
        let total_yield = base_yield.checked_add(compounding_bonus)
            .expect("Overflow")
            .checked_add(loyalty_bonus)
            .expect("Overflow");

        YieldPreview {
            base_yield,
            compounding_bonus,
            loyalty_bonus,
            total_yield,
            days_elapsed,
            days_remaining,
        }
    }

    /// Check if user can take action (liquidate or rollover)
    pub fn can_take_action(
        env: Env,
        user: Address,
    ) -> bool {
        // Check if position exists
        let position: Option<UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user));
        
        if position.is_none() {
            return false;
        }

        let position = position.unwrap();

        // Check if epoch complete
        let current_time = env.ledger().timestamp();
        let epoch_end = position.epoch_start.checked_add(EPOCH_DURATION)
            .unwrap_or(0);

        current_time >= epoch_end
    }

    /// Check if position is in grace period
    pub fn is_in_grace_period(
        env: Env,
        user: Address,
    ) -> bool {
        // Load position
        let position: Option<UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user));
        
        if position.is_none() {
            return false;
        }

        let position = position.unwrap();

        let current_time = env.ledger().timestamp();
        let epoch_end = position.epoch_start.checked_add(EPOCH_DURATION)
            .unwrap_or(0);
        let grace_period_end = epoch_end.checked_add(GRACE_PERIOD)
            .unwrap_or(0);

        current_time >= epoch_end && current_time < grace_period_end
    }

    /// Check if admin can rollover position
    pub fn can_admin_rollover(
        env: Env,
        user: Address,
    ) -> bool {
        // Load position
        let position: Option<UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::UserPosition(user));
        
        if position.is_none() {
            return false;
        }

        let position = position.unwrap();

        let current_time = env.ledger().timestamp();
        let grace_period_end = position.epoch_start
            .checked_add(EPOCH_DURATION)
            .unwrap_or(0)
            .checked_add(GRACE_PERIOD)
            .unwrap_or(0);

        current_time >= grace_period_end
    }

    /// Get property metadata
    pub fn get_metadata(env: &Env) -> PropertyMetadata {
        env.storage()
            .instance()
            .get(&METADATA_KEY)
            .expect("Property contract not initialized")
    }

    /// Get ROI configuration
    pub fn get_roi_config(env: &Env) -> RoiConfig {
        env.storage()
            .instance()
            .get(&ROI_CONFIG_KEY)
            .expect("Property contract not initialized")
    }

    /// Get total active tokens
    pub fn total_active_tokens(env: Env) -> i128 {
        env.storage().instance().get(&TOTAL_ACTIVE_KEY).unwrap_or(0)
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Property contract not initialized")
    }

    // Internal helper functions

    /// Calculate yield for a position
    fn calculate_yield(position: &UserPosition, roi_config: &RoiConfig) -> (i128, i128, i128) {
        // Base yield (monthly rate)
        let monthly_rate = roi_config.annual_rate_bps / 12;
        let base_yield = position.current_principal
            .checked_mul(monthly_rate as i128).unwrap_or(0)
            .checked_div(10_000).unwrap_or(0);
        
        // Compounding bonus
        let compounding_bonus = if position.compounding_enabled {
            let bonus_rate = roi_config.compounding_bonus_bps / 12;
            position.current_principal
                .checked_mul(bonus_rate as i128).unwrap_or(0)
                .checked_div(10_000).unwrap_or(0)
        } else {
            0
        };
        
        // Loyalty bonus
        let loyalty_rate = position.loyalty_tier * roi_config.loyalty_bonus_bps / 12;
        let loyalty_bonus = position.current_principal
            .checked_mul(loyalty_rate as i128).unwrap_or(0)
            .checked_div(10_000).unwrap_or(0);
        
        (base_yield, compounding_bonus, loyalty_bonus)
    }
}

// Import client types for cross-contract calls
mod kyc_contract {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/verse_kyc.wasm");
}
pub use kyc_contract::Client as KycContractClient;

mod vault_contract {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/verse_vault.wasm");
}
pub use vault_contract::Client as VaultContractClient;

mod test;
// Integration tests require proper WASM builds
// mod integration_test;

