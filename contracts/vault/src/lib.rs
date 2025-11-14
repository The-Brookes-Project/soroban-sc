#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Vec, symbol_short, Symbol,
};

// Storage keys
const CONFIG_KEY: Symbol = symbol_short!("CONFIG");
const AUTH_PROPS: Symbol = symbol_short!("AUTH_PRPS");
const QUEUE_HEAD: Symbol = symbol_short!("Q_HEAD");
const QUEUE_TAIL: Symbol = symbol_short!("Q_TAIL");

// Error codes
pub const ERR_ALREADY_INIT: u32 = 1;
pub const ERR_NOT_ADMIN: u32 = 2;
pub const ERR_INVALID_AMOUNT: u32 = 3;
pub const ERR_EMERGENCY_PAUSED: u32 = 4;
pub const ERR_NOT_AUTHORIZED: u32 = 5;
pub const ERR_INSUFFICIENT_FUNDS: u32 = 6;
pub const ERR_OVERFLOW: u32 = 7;
pub const ERR_ALREADY_AUTHORIZED: u32 = 8;
pub const ERR_NOT_FOUND: u32 = 9;

// Vault configuration
#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub admin: Address,
    pub stablecoin_address: Address,
    pub total_capacity: i128,
    pub available: i128,
    pub buffer_percentage: u32,      // 10-25%, stored as whole number (e.g., 15 = 15%)
    pub controlled_mode: bool,
    pub emergency_pause: bool,
}

// Liquidation request in queue
#[contracttype]
#[derive(Clone, Debug)]
pub struct LiquidationRequest {
    pub request_id: u64,
    pub property: Address,
    pub user: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub estimated_fulfill_date: u64,
}

// Queue status for view function
#[contracttype]
#[derive(Clone, Debug)]
pub struct QueueStatus {
    pub total_queued: u32,
    pub total_amount: i128,
    pub controlled_mode: bool,
    pub head_index: u64,
    pub tail_index: u64,
    pub estimated_clear_time: u64,
}

// Property stats
#[contracttype]
#[derive(Clone, Debug)]
pub struct PropertyVaultStats {
    pub property_contract: Address,
    pub total_liquidated: i128,
    pub last_liquidation: u64,
    pub active_users: u32,
    pub cash_flow_monthly: i128,
}

// Storage key types for user-specific data
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    QueuedRequest(u64),     // request_id -> LiquidationRequest
    PropertyStats(Address),  // property_address -> PropertyVaultStats
}

// Event types
#[contracttype]
#[derive(Clone, Debug)]
pub enum VaultEvent {
    Initialized(Address),
    Funded(Address, i128, i128),                  // admin, amount, new_total
    PropertyAuthorized(Address, Address),         // admin, property
    LiquidationExecuted(Address, Address, i128),  // property, user, amount
    LiquidationQueued(u64, Address, Address, i128), // request_id, property, user, amount
    ControlledModeActivated(u64),                 // timestamp
    ControlledModeDeactivated(u64),               // timestamp
    LiquidityWithdrawn(Address, i128, i128),     // admin, amount, remaining
    EmergencyPaused(Address, u64),               // admin, timestamp
    EmergencyUnpaused(Address, u64),             // admin, timestamp
    BufferAdjusted(u32),                         // new_percentage
}

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// Initialize vault with admin and stablecoin address
    pub fn initialize(
        env: Env,
        admin: Address,
        stablecoin_address: Address,
    ) {
        admin.require_auth();

        // Check if already initialized
        if env.storage().instance().has(&CONFIG_KEY) {
            panic!("Vault already initialized");
        }

        // Prevent setting contract's own address as stablecoin
        if stablecoin_address == env.current_contract_address() {
            panic!("Stablecoin cannot be the contract itself");
        }

        // Create configuration
        let config = VaultConfig {
            admin: admin.clone(),
            stablecoin_address,
            total_capacity: 0,
            available: 0,
            buffer_percentage: 15, // Default 15%
            controlled_mode: false,
            emergency_pause: false,
        };

        // Store configuration
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Initialize empty authorized properties list
        let authorized: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&AUTH_PROPS, &authorized);

        // Initialize queue indices
        env.storage().instance().set(&QUEUE_HEAD, &0u64);
        env.storage().instance().set(&QUEUE_TAIL, &0u64);

        // Emit event
        env.events().publish(
            (symbol_short!("init"),),
            VaultEvent::Initialized(admin),
        );
    }

    /// Admin deposits USDC to fund the vault
    pub fn fund_vault(
        env: Env,
        admin: Address,
        amount: i128,
    ) {
        admin.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }
        if amount <= 0 {
            panic!("Invalid amount");
        }
        if config.emergency_pause {
            panic!("Emergency paused");
        }

        // Transfer USDC from admin to vault
        let token_client = token::Client::new(&env, &config.stablecoin_address);
        
        // Get balances before transfer for verification
        let vault_balance_before = token_client.balance(&env.current_contract_address());
        let admin_balance_before = token_client.balance(&admin);
        
        // Verify admin has sufficient balance
        if admin_balance_before < amount {
            panic!("Insufficient admin balance");
        }

        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        // Verify transfer succeeded
        let vault_balance_after = token_client.balance(&env.current_contract_address());
        let expected_vault_balance = vault_balance_before.checked_add(amount)
            .expect("Overflow in balance calculation");
        
        if vault_balance_after != expected_vault_balance {
            panic!("Transfer verification failed");
        }

        // Update vault state
        config.total_capacity = config.total_capacity.checked_add(amount)
            .expect("Overflow in total_capacity");
        config.available = config.available.checked_add(amount)
            .expect("Overflow in available");

        env.storage().instance().set(&CONFIG_KEY, &config);

        // Process any pending liquidations if now sufficient
        if config.controlled_mode {
            Self::attempt_process_queue(&env);
        }

        // Emit event
        env.events().publish(
            (symbol_short!("funded"),),
            VaultEvent::Funded(admin, amount, config.total_capacity),
        );
    }

    /// Admin authorizes a property contract to request liquidations
    pub fn authorize_property(
        env: Env,
        admin: Address,
        property_contract: Address,
    ) {
        admin.require_auth();

        // Load configuration
        let config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }

        // Load authorized properties list
        let mut authorized: Vec<Address> = env.storage()
            .instance()
            .get(&AUTH_PROPS)
            .unwrap_or(Vec::new(&env));

        // Check not already authorized
        for prop in authorized.iter() {
            if prop == property_contract {
                panic!("Already authorized");
            }
        }

        // Add to authorized list
        authorized.push_back(property_contract.clone());
        env.storage().instance().set(&AUTH_PROPS, &authorized);

        // Initialize stats for this property
        let stats = PropertyVaultStats {
            property_contract: property_contract.clone(),
            total_liquidated: 0,
            last_liquidation: 0,
            active_users: 0,
            cash_flow_monthly: 0,
        };
        env.storage().persistent().set(
            &DataKey::PropertyStats(property_contract.clone()),
            &stats,
        );

        // Emit event
        env.events().publish(
            (symbol_short!("auth_prop"),),
            VaultEvent::PropertyAuthorized(admin, property_contract),
        );
    }

    /// Admin withdraws excess liquidity from vault
    pub fn withdraw_liquidity(
        env: Env,
        admin: Address,
        amount: i128,
    ) {
        admin.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }
        if amount <= 0 {
            panic!("Invalid amount");
        }
        if config.emergency_pause {
            panic!("Emergency paused");
        }

        // Calculate minimum required (buffer + queue obligations)
        let buffer_amount = config.total_capacity
            .checked_mul(config.buffer_percentage as i128)
            .expect("Overflow")
            .checked_div(100)
            .expect("Division error");
        
        let queue_obligations = Self::calculate_queue_obligations(&env);
        let min_required = buffer_amount.checked_add(queue_obligations)
            .expect("Overflow in min_required");

        // Check sufficient available after withdrawal
        let available_after = config.available.checked_sub(amount)
            .expect("Insufficient funds");
        
        if available_after < min_required {
            panic!("Would violate buffer requirements");
        }

        // Transfer USDC from vault to admin
        let token_client = token::Client::new(&env, &config.stablecoin_address);
        
        // Get balances before for verification
        let vault_balance_before = token_client.balance(&env.current_contract_address());
        
        if vault_balance_before < amount {
            panic!("Insufficient vault balance");
        }

        token_client.transfer(&env.current_contract_address(), &admin, &amount);

        // Verify transfer
        let vault_balance_after = token_client.balance(&env.current_contract_address());
        let expected_vault_balance = vault_balance_before.checked_sub(amount)
            .expect("Overflow");
        
        if vault_balance_after != expected_vault_balance {
            panic!("Withdrawal verification failed");
        }

        // Update vault state
        config.available = available_after;
        config.total_capacity = config.total_capacity.checked_sub(amount)
            .expect("Overflow in total_capacity");

        env.storage().instance().set(&CONFIG_KEY, &config);

        // Emit event
        env.events().publish(
            (symbol_short!("withdrawn"),),
            VaultEvent::LiquidityWithdrawn(admin, amount, config.available),
        );
    }

    /// Emergency pause - stops all liquidation processing
    pub fn emergency_pause(
        env: Env,
        admin: Address,
    ) {
        admin.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }
        if config.emergency_pause {
            panic!("Already paused");
        }

        // Set pause flag
        config.emergency_pause = true;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Emit event
        env.events().publish(
            (symbol_short!("paused"),),
            VaultEvent::EmergencyPaused(admin, env.ledger().timestamp()),
        );
    }

    /// Emergency unpause - resumes liquidation processing
    pub fn emergency_unpause(
        env: Env,
        admin: Address,
    ) {
        admin.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }
        if !config.emergency_pause {
            panic!("Not paused");
        }

        // Clear pause flag
        config.emergency_pause = false;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Try to process queue
        if config.controlled_mode {
            Self::attempt_process_queue(&env);
        }

        // Emit event
        env.events().publish(
            (symbol_short!("unpaused"),),
            VaultEvent::EmergencyUnpaused(admin, env.ledger().timestamp()),
        );
    }

    /// Update buffer percentage (admin only)
    pub fn update_buffer_percentage(
        env: Env,
        admin: Address,
        new_percentage: u32,
    ) {
        admin.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if admin != config.admin {
            panic!("Not admin");
        }
        if new_percentage < 10 || new_percentage > 25 {
            panic!("Buffer must be between 10-25%");
        }

        // Update buffer
        config.buffer_percentage = new_percentage;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Emit event
        env.events().publish(
            (symbol_short!("buffer"),),
            VaultEvent::BufferAdjusted(new_percentage),
        );
    }

    /// Property contract requests liquidation for a user
    pub fn request_liquidation(
        env: Env,
        property_contract: Address,
        user: Address,
        amount: i128,
    ) {
        property_contract.require_auth();

        // Load configuration
        let mut config = Self::get_config(&env);

        // Validate
        if config.emergency_pause {
            panic!("Emergency paused");
        }
        if amount <= 0 {
            panic!("Invalid amount");
        }

        // Check property is authorized
        let authorized: Vec<Address> = env.storage()
            .instance()
            .get(&AUTH_PROPS)
            .unwrap_or(Vec::new(&env));
        
        let mut is_authorized = false;
        for prop in authorized.iter() {
            if prop == property_contract {
                is_authorized = true;
                break;
            }
        }
        
        if !is_authorized {
            panic!("Property not authorized");
        }

        // Calculate buffer threshold
        let buffer_threshold = config.total_capacity
            .checked_mul(config.buffer_percentage as i128)
            .expect("Overflow")
            .checked_div(100)
            .expect("Division error");

        // Check if instant processing possible
        let required_available = buffer_threshold.checked_add(amount)
            .expect("Overflow");
        
        if config.available >= required_available && !config.controlled_mode {
            // INSTANT PROCESSING PATH
            let token_client = token::Client::new(&env, &config.stablecoin_address);
            
            // Verify vault has sufficient balance
            let vault_balance = token_client.balance(&env.current_contract_address());
            if vault_balance < amount {
                panic!("Insufficient vault balance");
            }

            token_client.transfer(&env.current_contract_address(), &user, &amount);

            // Update vault state
            config.available = config.available.checked_sub(amount)
                .expect("Overflow in available");
            env.storage().instance().set(&CONFIG_KEY, &config);

            // Update property stats
            let mut stats: PropertyVaultStats = env.storage()
                .persistent()
                .get(&DataKey::PropertyStats(property_contract.clone()))
                .unwrap_or(PropertyVaultStats {
                    property_contract: property_contract.clone(),
                    total_liquidated: 0,
                    last_liquidation: 0,
                    active_users: 0,
                    cash_flow_monthly: 0,
                });
            
            stats.total_liquidated = stats.total_liquidated.checked_add(amount)
                .expect("Overflow in stats");
            stats.last_liquidation = env.ledger().timestamp();
            env.storage().persistent().set(
                &DataKey::PropertyStats(property_contract.clone()),
                &stats,
            );

            // Emit event
            env.events().publish(
                (symbol_short!("liq_exec"),),
                VaultEvent::LiquidationExecuted(property_contract, user, amount),
            );
        } else {
            // QUEUING PATH - Enter controlled mode
            if !config.controlled_mode {
                config.controlled_mode = true;
                env.storage().instance().set(&CONFIG_KEY, &config);
                
                env.events().publish(
                    (symbol_short!("ctrl_mode"),),
                    VaultEvent::ControlledModeActivated(env.ledger().timestamp()),
                );
            }

            // Get tail index and increment it
            let tail_index: u64 = env.storage()
                .instance()
                .get(&QUEUE_TAIL)
                .unwrap_or(0);
            
            let request_id = tail_index;

            // Calculate estimated fulfillment date
            let estimated_fulfill_date = Self::estimate_fulfillment(&env, amount);

            // Create liquidation request
            let request = LiquidationRequest {
                request_id,
                property: property_contract.clone(),
                user: user.clone(),
                amount,
                timestamp: env.ledger().timestamp(),
                estimated_fulfill_date,
            };

            // Add to queue
            env.storage().persistent().set(
                &DataKey::QueuedRequest(request_id),
                &request,
            );

            // Update tail index
            let new_tail = tail_index.checked_add(1)
                .expect("Queue overflow");
            env.storage().instance().set(&QUEUE_TAIL, &new_tail);

            // Emit event
            env.events().publish(
                (symbol_short!("liq_queue"),),
                VaultEvent::LiquidationQueued(request_id, property_contract, user, amount),
            );
        }
    }

    // View functions

    /// Get current available liquidity
    pub fn available_liquidity(env: Env) -> i128 {
        let config = Self::get_config(&env);
        config.available
    }

    /// Get total vault capacity
    pub fn total_capacity(env: Env) -> i128 {
        let config = Self::get_config(&env);
        config.total_capacity
    }

    /// Check if property is authorized
    pub fn is_authorized(
        env: Env,
        property_contract: Address,
    ) -> bool {
        let authorized: Vec<Address> = env.storage()
            .instance()
            .get(&AUTH_PROPS)
            .unwrap_or(Vec::new(&env));
        
        for prop in authorized.iter() {
            if prop == property_contract {
                return true;
            }
        }
        false
    }

    /// Get vault configuration
    pub fn get_config(env: &Env) -> VaultConfig {
        env.storage()
            .instance()
            .get(&CONFIG_KEY)
            .expect("Vault not initialized")
    }

    /// Get liquidation queue status
    pub fn get_queue_status(env: Env) -> QueueStatus {
        let config = Self::get_config(&env);
        
        let head_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_HEAD)
            .unwrap_or(0);
        
        let tail_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_TAIL)
            .unwrap_or(0);

        // Calculate total queued amount
        let mut total_amount = 0i128;
        let mut total_queued = 0u32;
        
        for i in head_index..tail_index {
            if let Some(request) = env.storage()
                .persistent()
                .get::<DataKey, LiquidationRequest>(&DataKey::QueuedRequest(i)) 
            {
                total_amount = total_amount.checked_add(request.amount)
                    .expect("Overflow in queue calculation");
                total_queued = total_queued.checked_add(1)
                    .expect("Overflow in queue count");
            }
        }

        // Calculate estimated clear time
        let monthly_cash_flow = Self::calculate_expected_cash_flow(&env);
        let estimated_clear_time = if total_amount == 0 || monthly_cash_flow <= 0 {
            env.ledger().timestamp()
        } else {
            let months_needed = total_amount.checked_div(monthly_cash_flow).unwrap_or(1);
            let months_capped = if months_needed > 12 { 12 } else { months_needed };
            let seconds_to_add = months_capped.checked_mul(2_592_000).unwrap_or(2_592_000);
            env.ledger().timestamp().checked_add(seconds_to_add as u64).unwrap_or(0)
        };

        QueueStatus {
            total_queued,
            total_amount,
            controlled_mode: config.controlled_mode,
            head_index,
            tail_index,
            estimated_clear_time,
        }
    }

    /// Get property stats
    pub fn get_property_stats(
        env: Env,
        property_contract: Address,
    ) -> Option<PropertyVaultStats> {
        env.storage()
            .persistent()
            .get(&DataKey::PropertyStats(property_contract))
    }

    // Internal helper functions

    /// Calculate total obligations in queue
    fn calculate_queue_obligations(env: &Env) -> i128 {
        let head_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_HEAD)
            .unwrap_or(0);
        
        let tail_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_TAIL)
            .unwrap_or(0);

        let mut total = 0i128;
        for i in head_index..tail_index {
            if let Some(request) = env.storage()
                .persistent()
                .get::<DataKey, LiquidationRequest>(&DataKey::QueuedRequest(i))
            {
                total = total.checked_add(request.amount)
                    .expect("Overflow in obligations");
            }
        }
        total
    }

    /// Calculate expected monthly cash flow from all properties
    fn calculate_expected_cash_flow(env: &Env) -> i128 {
        let authorized: Vec<Address> = env.storage()
            .instance()
            .get(&AUTH_PROPS)
            .unwrap_or(Vec::new(env));
        
        let mut total_cash_flow = 0i128;
        for property in authorized.iter() {
            if let Some(stats) = env.storage()
                .persistent()
                .get::<DataKey, PropertyVaultStats>(&DataKey::PropertyStats(property))
            {
                total_cash_flow = total_cash_flow.checked_add(stats.cash_flow_monthly)
                    .unwrap_or(total_cash_flow);
            }
        }
        total_cash_flow
    }

    /// Estimate fulfillment date for a liquidation request
    fn estimate_fulfillment(env: &Env, amount: i128) -> u64 {
        let monthly_cash_flow = Self::calculate_expected_cash_flow(env);
        
        // If no cash flow, estimate far in the future (90 days)
        if monthly_cash_flow <= 0 {
            return env.ledger().timestamp().checked_add(7_776_000).unwrap_or(0); // 90 days
        }
        
        // Calculate months needed to accumulate this amount
        let months_needed = amount.checked_div(monthly_cash_flow).unwrap_or(1);
        
        // Cap at reasonable maximum (12 months)
        let months_capped = if months_needed > 12 { 12 } else { months_needed };
        
        // Calculate estimated date (months * 30 days in seconds)
        let seconds_to_add = months_capped.checked_mul(2_592_000).unwrap_or(2_592_000);
        
        env.ledger().timestamp().checked_add(seconds_to_add as u64).unwrap_or(0)
    }

    /// Attempt to process queued liquidations
    fn attempt_process_queue(env: &Env) {
        let mut config = Self::get_config(env);

        if !config.controlled_mode {
            return; // Not in controlled mode
        }

        let buffer_threshold = config.total_capacity
            .checked_mul(config.buffer_percentage as i128)
            .expect("Overflow")
            .checked_div(100)
            .expect("Division error");

        let head_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_HEAD)
            .unwrap_or(0);
        
        let tail_index: u64 = env.storage()
            .instance()
            .get(&QUEUE_TAIL)
            .unwrap_or(0);

        let mut current_head = head_index;
        let token_client = token::Client::new(env, &config.stablecoin_address);

        // Process queue in FIFO order
        for i in head_index..tail_index {
            if let Some(request) = env.storage()
                .persistent()
                .get::<DataKey, LiquidationRequest>(&DataKey::QueuedRequest(i))
            {
                // Check if sufficient liquidity
                let required_available = buffer_threshold.checked_add(request.amount)
                    .expect("Overflow");
                
                if config.available >= required_available {
                    // Process this request
                    token_client.transfer(
                        &env.current_contract_address(),
                        &request.user,
                        &request.amount,
                    );

                    // Update available
                    config.available = config.available.checked_sub(request.amount)
                        .expect("Overflow");

                    // Remove from queue
                    env.storage().persistent().remove(&DataKey::QueuedRequest(i));

                    // Update property stats
                    if let Some(mut stats) = env.storage()
                        .persistent()
                        .get::<DataKey, PropertyVaultStats>(&DataKey::PropertyStats(request.property.clone()))
                    {
                        stats.total_liquidated = stats.total_liquidated.checked_add(request.amount)
                            .expect("Overflow in stats");
                        stats.last_liquidation = env.ledger().timestamp();
                        env.storage().persistent().set(
                            &DataKey::PropertyStats(request.property.clone()),
                            &stats,
                        );
                    }

                    // Update head
                    current_head = i.checked_add(1).expect("Queue overflow");

                    // Emit event
                    env.events().publish(
                        (symbol_short!("liq_exec"),),
                        VaultEvent::LiquidationExecuted(
                            request.property,
                            request.user,
                            request.amount,
                        ),
                    );
                } else {
                    break; // Not enough for this one, stop processing
                }
            } else {
                // Request already processed, move head forward
                current_head = i.checked_add(1).expect("Queue overflow");
            }
        }

        // Update head index
        env.storage().instance().set(&QUEUE_HEAD, &current_head);

        // Check if queue is now empty
        if current_head >= tail_index {
            config.controlled_mode = false;
            env.events().publish(
                (symbol_short!("norm_mode"),),
                VaultEvent::ControlledModeDeactivated(env.ledger().timestamp()),
            );
        }

        env.storage().instance().set(&CONFIG_KEY, &config);
    }
}

mod test;
mod integration_test;

