#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Vec,
                  symbol_short, Error, Symbol};

// TTL constants (industry standard values)
// ~12 ledgers per minute, ~17280 ledgers per day
const INSTANCE_TTL_THRESHOLD: u32 = 535_680;   // ~31 days
const INSTANCE_TTL_EXTEND: u32 = 1_589_760;    // ~92 days
const PERSISTENT_TTL_THRESHOLD: u32 = 535_680;  // ~31 days
const PERSISTENT_TTL_EXTEND: u32 = 1_589_760;   // ~92 days

// Storage keys
const METADATA_KEY: Symbol = symbol_short!("METADATA");
const CONFIG_KEY: Symbol = symbol_short!("CONFIG");
const ADMINS_KEY: Symbol = symbol_short!("ADMINS");
const USDC_BAL_KEY: Symbol = symbol_short!("USDC_BAL");

// Business logic constants
const MAX_DECIMALS: u32 = 7;
const INITIAL_BALANCE: i128 = 0;
const DECIMAL_BASE: i128 = 10;
const MAX_TOTAL_SUPPLY: i128 = 1_000_000_000_000_000_000; // 1 quintillion
const MAX_USDC_PRICE: i128 = 1_000_000_000_000; // 1 trillion
const MAX_NAME_LEN: u32 = 64;
const MAX_SYMBOL_LEN: u32 = 12;
const MAX_HOME_DOMAIN_LEN: u32 = 256;

// Error codes
const ERR_INVALID_AMOUNT: u32 = 1;
const ERR_TRANSFER_RESTRICTED: u32 = 2;
const ERR_NOT_ADMIN_KYC: u32 = 3;
const ERR_NOT_ADMIN_COMPLIANCE: u32 = 4;
const ERR_NOT_ADMIN_CLAWBACK: u32 = 5;
const ERR_NOT_ADMIN_ADD_ADMIN: u32 = 8;
const ERR_DUPLICATE_ADMIN: u32 = 9;
const ERR_NOT_ADMIN_CONFIGURE_AUTH: u32 = 10;
const ERR_NOT_ADMIN_TRANSFER_RESTRICTION: u32 = 11;
const ERR_KYC_NOT_VERIFIED: u32 = 12;
const ERR_COMPLIANCE_NOT_APPROVED: u32 = 13;
const ERR_INSUFFICIENT_BALANCE: u32 = 14;
const ERR_INVALID_PURCHASE_AMOUNT: u32 = 15;
const ERR_CALCULATION_OVERFLOW: u32 = 16;
const ERR_INSUFFICIENT_ISSUER_TOKENS: u32 = 17;
const ERR_INVALID_WITHDRAW_AMOUNT: u32 = 19;
const ERR_INSUFFICIENT_USDC_BALANCE: u32 = 20;
const ERR_USDC_TRANSFER_VERIFICATION_FAILED: u32 = 21;
const ERR_INSUFFICIENT_USDC_IN_CONTRACT: u32 = 22;
const ERR_USDC_WITHDRAWAL_VERIFICATION_FAILED: u32 = 23;
const ERR_SELF_TRANSFER_NOT_ALLOWED: u32 = 24;
const ERR_AUTHORIZATION_NOT_REVOCABLE: u32 = 25;
const ERR_NOT_ISSUER: u32 = 26;
const ERR_CANNOT_REMOVE_ISSUER: u32 = 27;
const ERR_NOT_AN_ADMIN: u32 = 28;
const ERR_NOT_ADMIN_TTL: u32 = 29;

// Define token metadata structure
#[contracttype]
#[derive(Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub total_supply: i128,
    pub issuer: Address,
    pub home_domain: String,
    pub usdc_price: i128, // Price in USDC per token (in the smallest unit)
    pub usdc_token: Address, // USDC token contract address
}

// Define contract configuration
#[contracttype]
#[derive(Clone)]
pub struct ContractConfig {
    pub authorization_required: bool,
    pub authorization_revocable: bool,
    pub transfer_restricted: bool,
}

// Define compliance status enum
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ComplianceStatus {
    Pending,
    Approved,
    Rejected,
    Suspended,
}

// Storage key types for user-specific data
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Balance(Address),
    KycVerified(Address),
    ComplianceStatus(Address),
}

// Define event types that the contract will emit - using tuple variants
#[contracttype]
pub enum SecurityTokenEvent {
    Init(TokenMetadata),
    Transfer(Address, Address, i128), // from, to, amount
    ComplianceUpdated(Address, ComplianceStatus), // address, status
    KycVerified(Address, bool), // address, status
    AuthorizationChanged(bool, bool), // required, revocable
    ClawbackExecuted(Address, i128), // from, amount
    Purchase(Address, Address, i128, i128), // buyer, beneficiary, token_amount, usdc_amount
    UsdcWithdrawn(Address, i128), // admin, amount
    AdminAdded(Address, Address), // admin, new_admin
    AdminRemoved(Address, Address), // issuer, removed_admin
    TransferRestrictionChanged(bool), // restricted status
}

// Main contract
#[contract]
pub struct SecurityTokenContract;

#[contractimpl]
impl SecurityTokenContract {
    // Constructor to initialize the token with required parameters (Protocol 22+)
    // Runs once during contract deployment, preventing front-running attacks
    pub fn __constructor(
        env: Env,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
        issuer: Address,
        home_domain: String,
        admin: Address,
        usdc_price: i128,
        usdc_token: Address,
    ) {
        // Require authorization from the admin who is initializing
        admin.require_auth();

        // Validate parameters
        if total_supply <= 0 {
            panic!("Total supply must be positive");
        }
        if total_supply > MAX_TOTAL_SUPPLY {
            panic!("Total supply cannot exceed 1 quintillion");
        }
        if decimals > MAX_DECIMALS {
            panic!("Decimals cannot exceed 7");
        }
        if usdc_price <= 0 {
            panic!("USDC price must be positive");
        }
        if usdc_price > MAX_USDC_PRICE {
            panic!("USDC price cannot exceed 1 trillion");
        }
        if home_domain.len() == 0 {
            panic!("Home domain cannot be empty");
        }
        if home_domain.len() > MAX_HOME_DOMAIN_LEN {
            panic!("Home domain cannot exceed 256 characters");
        }
        if name.len() == 0 {
            panic!("Name cannot be empty");
        }
        if name.len() > MAX_NAME_LEN {
            panic!("Name cannot exceed 64 characters");
        }
        if symbol.len() == 0 {
            panic!("Symbol cannot be empty");
        }
        if symbol.len() > MAX_SYMBOL_LEN {
            panic!("Symbol cannot exceed 12 characters");
        }

        // Validate USDC token address
        // Prevent setting the contract's own address as USDC token
        if usdc_token == env.current_contract_address() {
            panic!("USDC token cannot be the contract itself");
        }

        // Create and store token metadata in INSTANCE storage (small, fixed size)
        let metadata = TokenMetadata {
            name,
            symbol,
            decimals,
            total_supply,
            issuer: issuer.clone(),
            home_domain,
            usdc_price,
            usdc_token,
        };
        env.storage().instance().set(&METADATA_KEY, &metadata);

        // Create and store contract configuration in INSTANCE storage
        let config = ContractConfig {
            authorization_required: true,
            authorization_revocable: true,
            transfer_restricted: true,
        };
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Create and store initial admin list in INSTANCE storage
        let mut admins = Vec::new(&env);
        admins.push_back(admin.clone());
        admins.push_back(issuer.clone());
        env.storage().instance().set(&ADMINS_KEY, &admins);

        // Initialize USDC balance in INSTANCE storage
        env.storage().instance().set(&USDC_BAL_KEY, &INITIAL_BALANCE);

        // Assign total supply to issuer in PERSISTENT storage (user-specific data)
        let issuer_balance_key = DataKey::Balance(issuer.clone());
        env.storage()
            .persistent()
            .set(&issuer_balance_key, &total_supply);

        // Extend TTLs for all storage entries
        Self::extend_instance_ttl(&env);
        Self::extend_persistent_ttl(&env, &issuer_balance_key);

        // Auto-approve issuer for KYC and compliance since they're the token creator
        env.storage()
            .persistent()
            .set(&DataKey::KycVerified(issuer.clone()), &true);
        env.storage()
            .persistent()
            .set(&DataKey::ComplianceStatus(issuer.clone()), &ComplianceStatus::Approved);

        // Emit initialization event
        env.events().publish(
            (symbol_short!("init"),),
            SecurityTokenEvent::Init(metadata),
        );
    }

    // Transfer tokens between addresses with compliance checks
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(Error::from_contract_error(ERR_INVALID_AMOUNT));
        }

        // Load config from instance storage
        let config = Self::get_config(&env);

        // Check if transfers are currently allowed
        if config.transfer_restricted {
            // Only admins can transfer when restricted
            if !Self::is_admin(&env, &from) {
                return Err(Error::from_contract_error(ERR_TRANSFER_RESTRICTED));
            }
        }

        // Check compliance requirements
        Self::check_compliance_requirements(&env, &config, &from, &to)?;

        // Execute the transfer
        Self::execute_transfer(&env, &from, &to, amount)?;

        // Emit transfer event
        env.events().publish(
            (symbol_short!("transfer"),),
            SecurityTokenEvent::Transfer(from.clone(), to.clone(), amount),
        );

        Ok(())
    }

    // Set KYC verification status for an address
    pub fn set_kyc_status(
        env: Env,
        caller: Address,
        address: Address,
        verified: bool,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_KYC));
        }

        // Check if authorization is revocable when attempting to revoke
        let config = Self::get_config(&env);
        if !config.authorization_revocable && !verified {
            // Get current KYC status
            let current_kyc: bool = env
                .storage()
                .persistent()
                .get(&DataKey::KycVerified(address.clone()))
                .unwrap_or(false);
            
            // If currently verified and trying to revoke, check if revocation is allowed
            if current_kyc {
                return Err(Error::from_contract_error(ERR_AUTHORIZATION_NOT_REVOCABLE));
            }
        }

        // Update KYC status in PERSISTENT storage
        let kyc_key = DataKey::KycVerified(address.clone());
        env.storage()
            .persistent()
            .set(&kyc_key, &verified);

        // Extend TTL for the KYC entry
        Self::extend_persistent_ttl(&env, &kyc_key);

        // Emit event
        env.events().publish(
            (symbol_short!("kyc"),),
            SecurityTokenEvent::KycVerified(address.clone(), verified),
        );

        Ok(())
    }

    // Set compliance status for an address
    pub fn set_compliance_status(
        env: Env,
        caller: Address,
        address: Address,
        status: ComplianceStatus,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_COMPLIANCE));
        }

        // Check if authorization is revocable when attempting to downgrade from Approved
        let config = Self::get_config(&env);
        if !config.authorization_revocable && status != ComplianceStatus::Approved {
            // Get current compliance status
            let current_status: ComplianceStatus = env
                .storage()
                .persistent()
                .get(&DataKey::ComplianceStatus(address.clone()))
                .unwrap_or(ComplianceStatus::Pending);
            
            // If currently approved and trying to change to non-approved, check if revocation is allowed
            if current_status == ComplianceStatus::Approved {
                return Err(Error::from_contract_error(ERR_AUTHORIZATION_NOT_REVOCABLE));
            }
        }

        // Update compliance status in PERSISTENT storage
        let compliance_key = DataKey::ComplianceStatus(address.clone());
        env.storage()
            .persistent()
            .set(&compliance_key, &status);

        // Extend TTL for the compliance entry
        Self::extend_persistent_ttl(&env, &compliance_key);

        // Emit event
        env.events().publish(
            (symbol_short!("complianc"),),
            SecurityTokenEvent::ComplianceUpdated(address.clone(), status),
        );

        Ok(())
    }

    // Execute clawback of tokens (regulatory action)
    pub fn clawback(
        env: Env,
        caller: Address,
        from: Address,
        amount: i128,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_CLAWBACK));
        }

        // Validate amount is positive
        if amount <= 0 {
            return Err(Error::from_contract_error(ERR_INVALID_AMOUNT));
        }

        // Get current balance using helper
        let balance_key = DataKey::Balance(from.clone());
        let current_balance = Self::balance(env.clone(), from.clone());

        // Clawback the minimum of requested amount and available balance
        // This ensures we take what's available rather than failing if exact amount isn't present
        let actual_clawback_amount = if current_balance < amount {
            current_balance
        } else {
            amount
        };

        // Get issuer address from metadata
        let metadata = Self::get_metadata(&env);

        // Get issuer's current balance from PERSISTENT storage
        let issuer_balance_key = DataKey::Balance(metadata.issuer.clone());
        let issuer_balance: i128 = env
            .storage()
            .persistent()
            .get(&issuer_balance_key)
            .unwrap_or(INITIAL_BALANCE);

        // Update balances in PERSISTENT storage
        let new_balance = current_balance.checked_sub(actual_clawback_amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;

        let new_issuer_balance = issuer_balance.checked_add(actual_clawback_amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;

        env.storage()
            .persistent()
            .set(&balance_key, &new_balance);

        env.storage()
            .persistent()
            .set(&issuer_balance_key, &new_issuer_balance);

        // Extend TTLs for the balance entries
        Self::extend_persistent_ttl(&env, &balance_key);
        Self::extend_persistent_ttl(&env, &issuer_balance_key);

        // Emit event with actual clawed back amount
        env.events().publish(
            (symbol_short!("clawback"),),
            SecurityTokenEvent::ClawbackExecuted(from.clone(), actual_clawback_amount),
        );

        Ok(())
    }

    // Add an admin to the token (issuer only)
    pub fn add_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is issuer (only issuer can add admins)
        if !Self::is_issuer(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_ADD_ADMIN));
        }

        // Check if already an admin using helper
        if Self::is_admin(&env, &new_admin) {
            return Err(Error::from_contract_error(ERR_DUPLICATE_ADMIN));
        }

        // Get current admin list from INSTANCE storage
        let mut admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&ADMINS_KEY)
            .unwrap();

        // Add to admin list
        admins.push_back(new_admin.clone());
        env.storage().instance().set(&ADMINS_KEY, &admins);

        // Extend instance TTL
        Self::extend_instance_ttl(&env);

        // Emit admin added event
        env.events().publish(
            (symbol_short!("admin"),),
            SecurityTokenEvent::AdminAdded(caller.clone(), new_admin),
        );

        Ok(())
    }

    // Remove an admin from the token
    pub fn remove_admin(env: Env, caller: Address, admin_to_remove: Address) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is issuer
        if !Self::is_issuer(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ISSUER));
        }

        // Check if trying to remove the issuer
        if Self::is_issuer(&env, &admin_to_remove) {
            return Err(Error::from_contract_error(ERR_CANNOT_REMOVE_ISSUER));
        }

        // Check if the address is actually an admin
        if !Self::is_admin(&env, &admin_to_remove) {
            return Err(Error::from_contract_error(ERR_NOT_AN_ADMIN));
        }

        // Get current admin list from INSTANCE storage
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&ADMINS_KEY)
            .unwrap();

        // Remove the admin from the list
        let mut new_admins = Vec::new(&env);
        for admin in admins.iter() {
            if &admin != &admin_to_remove {
                new_admins.push_back(admin);
            }
        }

        // Update storage with new admin list
        env.storage().instance().set(&ADMINS_KEY, &new_admins);

        // Emit admin removed event
        env.events().publish(
            (symbol_short!("adminrem"),),
            SecurityTokenEvent::AdminRemoved(caller.clone(), admin_to_remove),
        );

        Ok(())
    }

    // Configure authorization flags
    pub fn configure_authorization(
        env: Env,
        caller: Address,
        required: bool,
        revocable: bool,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_CONFIGURE_AUTH));
        }

        // Update configuration in INSTANCE storage
        let mut config = Self::get_config(&env);
        config.authorization_required = required;
        config.authorization_revocable = revocable;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Extend instance TTL
        Self::extend_instance_ttl(&env);

        // Emit event
        env.events().publish(
            (symbol_short!("auth"),),
            SecurityTokenEvent::AuthorizationChanged(required, revocable),
        );

        Ok(())
    }

    // Direct purchase tokens with USDC
    pub fn purchase(
        env: Env,
        buyer: Address,
        beneficiary: Address,
        token_amount: i128,
    ) -> Result<(), Error> {
        buyer.require_auth();

        // Validate amount
        if token_amount <= 0 {
            return Err(Error::from_contract_error(ERR_INVALID_PURCHASE_AMOUNT));
        }

        // Load metadata from instance storage
        let metadata = Self::get_metadata(&env);

        // Check KYC and compliance status for buyer and beneficiary
        let config = Self::get_config(&env);
        Self::check_compliance_requirements(&env, &config, &metadata.issuer, &buyer)?;
        Self::check_compliance_requirements(&env, &config, &metadata.issuer, &beneficiary)?;

        // Calculate USDC amount needed
        let decimals_pow = DECIMAL_BASE.checked_pow(metadata.decimals)
            .ok_or(Error::from_contract_error(ERR_CALCULATION_OVERFLOW))?;

        let usdc_amount = token_amount.checked_mul(metadata.usdc_price)
            .ok_or(Error::from_contract_error(ERR_CALCULATION_OVERFLOW))?
            .checked_div(decimals_pow)
            .ok_or(Error::from_contract_error(ERR_CALCULATION_OVERFLOW))?;

        if usdc_amount <= 0 {
            return Err(Error::from_contract_error(ERR_CALCULATION_OVERFLOW));
        }

        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &metadata.usdc_token);

        // Verify buyer has sufficient USDC balance BEFORE transfer
        let buyer_usdc_balance_before = usdc_token_client.balance(&buyer);
        if buyer_usdc_balance_before < usdc_amount {
            return Err(Error::from_contract_error(ERR_INSUFFICIENT_USDC_BALANCE));
        }

        // Get contract's initial USDC balance for verification
        let contract_usdc_balance_before = usdc_token_client.balance(&env.current_contract_address());

        // Transfer USDC from buyer to contract
        usdc_token_client.transfer(&buyer, &env.current_contract_address(), &usdc_amount);

        // Verify the transfer actually occurred by checking balances
        let buyer_usdc_balance_after = usdc_token_client.balance(&buyer);
        let contract_usdc_balance_after = usdc_token_client.balance(&env.current_contract_address());

        // Verify buyer's balance decreased by the expected amount
        let expected_buyer_balance = buyer_usdc_balance_before.checked_sub(usdc_amount)
            .ok_or(Error::from_contract_error(ERR_USDC_TRANSFER_VERIFICATION_FAILED))?;
        
        if buyer_usdc_balance_after != expected_buyer_balance {
            return Err(Error::from_contract_error(ERR_USDC_TRANSFER_VERIFICATION_FAILED));
        }

        // Verify contract's balance increased by the expected amount
        let expected_contract_balance = contract_usdc_balance_before.checked_add(usdc_amount)
            .ok_or(Error::from_contract_error(ERR_USDC_TRANSFER_VERIFICATION_FAILED))?;
        
        if contract_usdc_balance_after != expected_contract_balance {
            return Err(Error::from_contract_error(ERR_USDC_TRANSFER_VERIFICATION_FAILED));
        }

        // Get balances using helper functions
        let issuer_balance_key = DataKey::Balance(metadata.issuer.clone());
        let beneficiary_balance_key = DataKey::Balance(beneficiary.clone());

        let issuer_balance = Self::balance(env.clone(), metadata.issuer.clone());
        let beneficiary_balance = Self::balance(env.clone(), beneficiary.clone());

        // Check if issuer has enough tokens
        if issuer_balance < token_amount {
            return Err(Error::from_contract_error(ERR_INSUFFICIENT_ISSUER_TOKENS));
        }

        // Update token balances in PERSISTENT storage
        let new_issuer_balance = issuer_balance.checked_sub(token_amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;
        let new_beneficiary_balance = beneficiary_balance.checked_add(token_amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;

        env.storage()
            .persistent()
            .set(&issuer_balance_key, &new_issuer_balance);
        env.storage()
            .persistent()
            .set(&beneficiary_balance_key, &new_beneficiary_balance);

        // Extend TTLs for issuer and beneficiary balances
        Self::extend_persistent_ttl(&env, &issuer_balance_key);
        Self::extend_persistent_ttl(&env, &beneficiary_balance_key);

        // Update USDC balance using helper
        let current_usdc_balance = Self::usdc_balance(env.clone());
        let new_usdc_balance = current_usdc_balance.checked_add(usdc_amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;
        env.storage().instance().set(&USDC_BAL_KEY, &new_usdc_balance);

        // Emit purchase event
        env.events().publish(
            (symbol_short!("purchase"),),
            SecurityTokenEvent::Purchase(buyer.clone(), beneficiary.clone(), token_amount, usdc_amount),
        );

        Ok(())
    }

    // Issuer-only function to withdraw accumulated USDC
    pub fn withdraw_usdc(
        env: Env,
        caller: Address,
        amount: i128,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is issuer (only issuer can withdraw USDC)
        if !Self::is_issuer(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ISSUER));
        }

        // Get USDC balance using helper
        let usdc_balance = Self::usdc_balance(env.clone());

        // Validate amount
        if amount <= 0 || amount > usdc_balance {
            return Err(Error::from_contract_error(ERR_INVALID_WITHDRAW_AMOUNT));
        }

        // Get metadata for USDC token address
        let metadata = Self::get_metadata(&env);
        let usdc_token_client = token::Client::new(&env, &metadata.usdc_token);

        // Get initial balances for verification
        let contract_usdc_balance_before = usdc_token_client.balance(&env.current_contract_address());
        let admin_usdc_balance_before = usdc_token_client.balance(&caller);

        // Verify contract has sufficient USDC before withdrawal
        if contract_usdc_balance_before < amount {
            return Err(Error::from_contract_error(ERR_INSUFFICIENT_USDC_IN_CONTRACT));
        }

        // Transfer USDC from contract to admin
        usdc_token_client.transfer(&env.current_contract_address(), &caller, &amount);

        // Verify the transfer actually occurred by checking balances
        let contract_usdc_balance_after = usdc_token_client.balance(&env.current_contract_address());
        let admin_usdc_balance_after = usdc_token_client.balance(&caller);

        // Verify contract's balance decreased by the expected amount
        let expected_contract_balance = contract_usdc_balance_before.checked_sub(amount)
            .ok_or(Error::from_contract_error(ERR_USDC_WITHDRAWAL_VERIFICATION_FAILED))?;
        
        if contract_usdc_balance_after != expected_contract_balance {
            return Err(Error::from_contract_error(ERR_USDC_WITHDRAWAL_VERIFICATION_FAILED));
        }

        // Verify admin's balance increased by the expected amount
        let expected_admin_balance = admin_usdc_balance_before.checked_add(amount)
            .ok_or(Error::from_contract_error(ERR_USDC_WITHDRAWAL_VERIFICATION_FAILED))?;
        
        if admin_usdc_balance_after != expected_admin_balance {
            return Err(Error::from_contract_error(ERR_USDC_WITHDRAWAL_VERIFICATION_FAILED));
        }

        // Update USDC balance in INSTANCE storage
        let new_usdc_balance = usdc_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;
        env.storage().instance().set(&USDC_BAL_KEY, &new_usdc_balance);

        // Extend instance TTL
        Self::extend_instance_ttl(&env);

        // Emit withdrawal event
        env.events().publish(
            (symbol_short!("withdraw"),),
            SecurityTokenEvent::UsdcWithdrawn(caller.clone(), amount),
        );

        Ok(())
    }

    // Set transfer restriction flag
    pub fn set_transfer_restriction(
        env: Env,
        caller: Address,
        restricted: bool,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_TRANSFER_RESTRICTION));
        }

        // Update configuration in INSTANCE storage
        let mut config = Self::get_config(&env);
        config.transfer_restricted = restricted;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Extend instance TTL
        Self::extend_instance_ttl(&env);

        // Emit transfer restriction changed event
        env.events().publish(
            (symbol_short!("restrict"),),
            SecurityTokenEvent::TransferRestrictionChanged(restricted),
        );

        Ok(())
    }

    // Admin function to extend instance storage TTL on-demand
    pub fn bump_instance_ttl(env: Env, caller: Address) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_TTL));
        }

        Self::extend_instance_ttl(&env);

        Ok(())
    }

    // Admin function to extend TTLs for multiple user addresses (bulk operation)
    pub fn bump_user_ttls(env: Env, caller: Address, addresses: Vec<Address>) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(ERR_NOT_ADMIN_TTL));
        }

        for address in addresses.iter() {
            // Extend balance TTL if exists
            let balance_key = DataKey::Balance(address.clone());
            Self::extend_persistent_ttl(&env, &balance_key);

            // Extend KYC TTL if exists
            let kyc_key = DataKey::KycVerified(address.clone());
            Self::extend_persistent_ttl(&env, &kyc_key);

            // Extend compliance TTL if exists
            let compliance_key = DataKey::ComplianceStatus(address.clone());
            Self::extend_persistent_ttl(&env, &compliance_key);
        }

        Ok(())
    }

    // View function to get token metadata
    pub fn get_metadata(env: &Env) -> TokenMetadata {
        env.storage()
            .instance()
            .get(&METADATA_KEY)
            .expect("Token not initialized")
    }

    // View function to get balance
    pub fn balance(env: Env, address: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(address))
            .unwrap_or(INITIAL_BALANCE)
    }

    // View function to check compliance status
    pub fn check_compliance(env: Env, address: Address) -> ComplianceStatus {
        env.storage()
            .persistent()
            .get(&DataKey::ComplianceStatus(address))
            .unwrap_or(ComplianceStatus::Pending)
    }

    // View function to check KYC status
    pub fn is_kyc_verified(env: Env, address: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::KycVerified(address))
            .unwrap_or(false)
    }

    // View function to check accumulated USDC balance
    pub fn usdc_balance(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&USDC_BAL_KEY)
            .unwrap_or(INITIAL_BALANCE)
    }

    // View function to get token price in USDC
    pub fn token_price(env: Env) -> i128 {
        let metadata = Self::get_metadata(&env);
        metadata.usdc_price
    }

    // View function to get the issuer address
    pub fn get_issuer(env: Env) -> Address {
        let metadata = Self::get_metadata(&env);
        metadata.issuer
    }

    // Internal helper functions

    // Helper to extend instance storage TTL
    fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
    }

    // Helper to extend persistent storage TTL for a specific key
    fn extend_persistent_ttl(env: &Env, key: &DataKey) {
        if env.storage().persistent().has(key) {
            env.storage()
                .persistent()
                .extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
        }
    }

    // Helper to get config from storage
    fn get_config(env: &Env) -> ContractConfig {
        env.storage()
            .instance()
            .get(&CONFIG_KEY)
            .expect("Contract not initialized")
    }

    // Helper to check if address is an admin
    fn is_admin(env: &Env, address: &Address) -> bool {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&ADMINS_KEY)
            .unwrap_or(Vec::new(env));
        
        for admin in admins.iter() {
            if &admin == address {
                return true;
            }
        }
        false
    }

    // Helper to check if address is the issuer
    fn is_issuer(env: &Env, address: &Address) -> bool {
        let metadata = Self::get_metadata(env);
        &metadata.issuer == address
    }

    // Helper to check compliance requirements
    fn check_compliance_requirements(
        env: &Env,
        config: &ContractConfig,
        from: &Address,
        to: &Address,
    ) -> Result<(), Error> {
        // Check authorization required flag
        if config.authorization_required {
            // Check KYC status for both addresses
            let from_kyc = Self::is_kyc_verified(env.clone(), from.clone());
            let to_kyc = Self::is_kyc_verified(env.clone(), to.clone());

            if !from_kyc || !to_kyc {
                return Err(Error::from_contract_error(ERR_KYC_NOT_VERIFIED));
            }

            // Check compliance status for both addresses
            let from_compliance = Self::check_compliance(env.clone(), from.clone());
            let to_compliance = Self::check_compliance(env.clone(), to.clone());

            if from_compliance != ComplianceStatus::Approved
                || to_compliance != ComplianceStatus::Approved
            {
                return Err(Error::from_contract_error(ERR_COMPLIANCE_NOT_APPROVED));
            }
        }

        Ok(())
    }

    // Helper to execute transfer
    fn execute_transfer(
        env: &Env,
        from: &Address,
        to: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Prevent self-transfers to avoid balance manipulation
        if from == to {
            return Err(Error::from_contract_error(ERR_SELF_TRANSFER_NOT_ALLOWED));
        }

        let from_balance_key = DataKey::Balance(from.clone());
        let to_balance_key = DataKey::Balance(to.clone());

        // Get current balances using helper
        let from_balance = Self::balance(env.clone(), from.clone());
        let to_balance = Self::balance(env.clone(), to.clone());

        // Check if sender has enough balance
        if from_balance < amount {
            return Err(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE));
        }

        // Update balances in PERSISTENT storage
        let new_from_balance = from_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;
        let new_to_balance = to_balance.checked_add(amount)
            .ok_or(Error::from_contract_error(ERR_INSUFFICIENT_BALANCE))?;

        env.storage()
            .persistent()
            .set(&from_balance_key, &new_from_balance);
        env.storage()
            .persistent()
            .set(&to_balance_key, &new_to_balance);

        // Extend TTLs for both sender and receiver balances
        Self::extend_persistent_ttl(env, &from_balance_key);
        Self::extend_persistent_ttl(env, &to_balance_key);

        Ok(())
    }
}

mod test;
