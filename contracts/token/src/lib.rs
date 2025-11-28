#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Vec,
                  symbol_short, Error, Symbol};

// Storage keys
const METADATA_KEY: Symbol = symbol_short!("METADATA");
const CONFIG_KEY: Symbol = symbol_short!("CONFIG");
const ADMINS_KEY: Symbol = symbol_short!("ADMINS");
const USDC_BAL_KEY: Symbol = symbol_short!("USDC_BAL");

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
    pub clawback_enabled: bool,
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
    TransferRestrictionChanged(bool), // restricted status
}

// Main contract
#[contract]
pub struct SecurityTokenContract;

#[contractimpl]
impl SecurityTokenContract {
    // Initialize the token with required parameters
    pub fn initialize(
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

        // Ensure the contract is only initialized once
        if env.storage().instance().has(&METADATA_KEY) {
            panic!("Token already initialized");
        }

        // Validate parameters
        if total_supply <= 0 {
            panic!("Total supply must be positive");
        }
        if total_supply > 1_000_000_000_000_000_000 {
            panic!("Total supply cannot exceed 1 quintillion");
        }
        if decimals > 7 {
            panic!("Decimals cannot exceed 7");
        }
        if usdc_price <= 0 {
            panic!("USDC price must be positive");
        }
        if usdc_price > 1_000_000_000_000 {
            panic!("USDC price cannot exceed 1 trillion");
        }
        if home_domain.len() == 0 {
            panic!("Home domain cannot be empty");
        }
        if home_domain.len() > 256 {
            panic!("Home domain cannot exceed 256 characters");
        }
        if name.len() == 0 {
            panic!("Name cannot be empty");
        }
        if name.len() > 64 {
            panic!("Name cannot exceed 64 characters");
        }
        if symbol.len() == 0 {
            panic!("Symbol cannot be empty");
        }
        if symbol.len() > 12 {
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
            clawback_enabled: true,
            transfer_restricted: true,
        };
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Create and store initial admin list in INSTANCE storage
        let mut admins = Vec::new(&env);
        admins.push_back(admin.clone());
        admins.push_back(issuer.clone());
        env.storage().instance().set(&ADMINS_KEY, &admins);

        // Initialize USDC balance in INSTANCE storage
        env.storage().instance().set(&USDC_BAL_KEY, &0i128);

        // Assign total supply to issuer in PERSISTENT storage (user-specific data)
        env.storage()
            .persistent()
            .set(&DataKey::Balance(issuer.clone()), &total_supply);

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
            return Err(Error::from_contract_error(1));
        }

        // Load config from instance storage
        let config = Self::get_config(&env);

        // Check if transfers are currently allowed
        if config.transfer_restricted {
            // Only admins can transfer when restricted
            if !Self::is_admin(&env, &from) {
                return Err(Error::from_contract_error(2));
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
            return Err(Error::from_contract_error(3));
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
                return Err(Error::from_contract_error(25)); // Authorization not revocable
            }
        }

        // Update KYC status in PERSISTENT storage
        env.storage()
            .persistent()
            .set(&DataKey::KycVerified(address.clone()), &verified);

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
            return Err(Error::from_contract_error(4));
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
                return Err(Error::from_contract_error(25)); // Authorization not revocable
            }
        }

        // Update compliance status in PERSISTENT storage
        env.storage()
            .persistent()
            .set(&DataKey::ComplianceStatus(address.clone()), &status);

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
            return Err(Error::from_contract_error(5));
        }

        // Check if clawback is enabled
        let config = Self::get_config(&env);
        if !config.clawback_enabled {
            return Err(Error::from_contract_error(6));
        }

        // Get current balance from PERSISTENT storage
        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);

        if current_balance < amount {
            return Err(Error::from_contract_error(7));
        }

        // Update balance in PERSISTENT storage
        let new_balance = current_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;
        
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &new_balance);

        // Emit event
        env.events().publish(
            (symbol_short!("clawback"),),
            SecurityTokenEvent::ClawbackExecuted(from.clone(), amount),
        );

        Ok(())
    }

    // Add an admin to the token
    pub fn add_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(8));
        }

        // Get current admin list from INSTANCE storage
        let mut admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&ADMINS_KEY)
            .unwrap();

        // Check if already an admin
        for existing_admin in admins.iter() {
            if &existing_admin == &new_admin {
                return Err(Error::from_contract_error(9));
            }
        }

        // Add to admin list
        admins.push_back(new_admin.clone());
        env.storage().instance().set(&ADMINS_KEY, &admins);

        // Emit admin added event
        env.events().publish(
            (symbol_short!("admin"),),
            SecurityTokenEvent::AdminAdded(caller.clone(), new_admin),
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
            return Err(Error::from_contract_error(10));
        }

        // Update configuration in INSTANCE storage
        let mut config = Self::get_config(&env);
        config.authorization_required = required;
        config.authorization_revocable = revocable;
        env.storage().instance().set(&CONFIG_KEY, &config);

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
            return Err(Error::from_contract_error(15));
        }

        // Load metadata from instance storage
        let metadata = Self::get_metadata(&env);

        // Check KYC and compliance status for buyer and beneficiary
        let config = Self::get_config(&env);
        Self::check_compliance_requirements(&env, &config, &metadata.issuer, &buyer)?;
        Self::check_compliance_requirements(&env, &config, &metadata.issuer, &beneficiary)?;

        // Calculate USDC amount needed
        let decimals_pow = 10i128.checked_pow(metadata.decimals)
            .ok_or(Error::from_contract_error(16))?;

        let usdc_amount = token_amount.checked_mul(metadata.usdc_price)
            .ok_or(Error::from_contract_error(16))?
            .checked_div(decimals_pow)
            .ok_or(Error::from_contract_error(16))?;

        if usdc_amount <= 0 {
            return Err(Error::from_contract_error(16));
        }

        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &metadata.usdc_token);

        // Verify buyer has sufficient USDC balance BEFORE transfer
        let buyer_usdc_balance_before = usdc_token_client.balance(&buyer);
        if buyer_usdc_balance_before < usdc_amount {
            return Err(Error::from_contract_error(20)); // Insufficient USDC balance
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
            .ok_or(Error::from_contract_error(21))?; // USDC transfer verification failed
        
        if buyer_usdc_balance_after != expected_buyer_balance {
            return Err(Error::from_contract_error(21)); // USDC transfer verification failed
        }

        // Verify contract's balance increased by the expected amount
        let expected_contract_balance = contract_usdc_balance_before.checked_add(usdc_amount)
            .ok_or(Error::from_contract_error(21))?; // USDC transfer verification failed
        
        if contract_usdc_balance_after != expected_contract_balance {
            return Err(Error::from_contract_error(21)); // USDC transfer verification failed
        }

        // Get balances from PERSISTENT storage
        let issuer_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(metadata.issuer.clone()))
            .unwrap_or(0);

        let beneficiary_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(beneficiary.clone()))
            .unwrap_or(0);

        // Check if issuer has enough tokens
        if issuer_balance < token_amount {
            return Err(Error::from_contract_error(17));
        }

        // Update token balances in PERSISTENT storage
        let new_issuer_balance = issuer_balance.checked_sub(token_amount)
            .ok_or(Error::from_contract_error(14))?;
        let new_beneficiary_balance = beneficiary_balance.checked_add(token_amount)
            .ok_or(Error::from_contract_error(14))?;

        env.storage()
            .persistent()
            .set(&DataKey::Balance(metadata.issuer.clone()), &new_issuer_balance);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(beneficiary.clone()), &new_beneficiary_balance);

        // Update USDC balance in INSTANCE storage
        let current_usdc_balance: i128 = env
            .storage()
            .instance()
            .get(&USDC_BAL_KEY)
            .unwrap_or(0);
        let new_usdc_balance = current_usdc_balance.checked_add(usdc_amount)
            .ok_or(Error::from_contract_error(14))?;
        env.storage().instance().set(&USDC_BAL_KEY, &new_usdc_balance);

        // Emit purchase event
        env.events().publish(
            (symbol_short!("purchase"),),
            SecurityTokenEvent::Purchase(buyer.clone(), beneficiary.clone(), token_amount, usdc_amount),
        );

        Ok(())
    }

    // Admin function to withdraw accumulated USDC
    pub fn withdraw_usdc(
        env: Env,
        caller: Address,
        amount: i128,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Check if caller is admin
        if !Self::is_admin(&env, &caller) {
            return Err(Error::from_contract_error(18));
        }

        // Get USDC balance from INSTANCE storage
        let usdc_balance: i128 = env
            .storage()
            .instance()
            .get(&USDC_BAL_KEY)
            .unwrap_or(0);

        // Validate amount
        if amount <= 0 || amount > usdc_balance {
            return Err(Error::from_contract_error(19));
        }

        // Get metadata for USDC token address
        let metadata = Self::get_metadata(&env);
        let usdc_token_client = token::Client::new(&env, &metadata.usdc_token);

        // Get initial balances for verification
        let contract_usdc_balance_before = usdc_token_client.balance(&env.current_contract_address());
        let admin_usdc_balance_before = usdc_token_client.balance(&caller);

        // Verify contract has sufficient USDC before withdrawal
        if contract_usdc_balance_before < amount {
            return Err(Error::from_contract_error(22)); // Insufficient USDC in contract
        }

        // Transfer USDC from contract to admin
        usdc_token_client.transfer(&env.current_contract_address(), &caller, &amount);

        // Verify the transfer actually occurred by checking balances
        let contract_usdc_balance_after = usdc_token_client.balance(&env.current_contract_address());
        let admin_usdc_balance_after = usdc_token_client.balance(&caller);

        // Verify contract's balance decreased by the expected amount
        let expected_contract_balance = contract_usdc_balance_before.checked_sub(amount)
            .ok_or(Error::from_contract_error(23))?; // USDC withdrawal verification failed
        
        if contract_usdc_balance_after != expected_contract_balance {
            return Err(Error::from_contract_error(23)); // USDC withdrawal verification failed
        }

        // Verify admin's balance increased by the expected amount
        let expected_admin_balance = admin_usdc_balance_before.checked_add(amount)
            .ok_or(Error::from_contract_error(23))?; // USDC withdrawal verification failed
        
        if admin_usdc_balance_after != expected_admin_balance {
            return Err(Error::from_contract_error(23)); // USDC withdrawal verification failed
        }

        // Update USDC balance in INSTANCE storage
        let new_usdc_balance = usdc_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;
        env.storage().instance().set(&USDC_BAL_KEY, &new_usdc_balance);

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
            return Err(Error::from_contract_error(11));
        }

        // Update configuration in INSTANCE storage
        let mut config = Self::get_config(&env);
        config.transfer_restricted = restricted;
        env.storage().instance().set(&CONFIG_KEY, &config);

        // Emit transfer restriction changed event
        env.events().publish(
            (symbol_short!("restrict"),),
            SecurityTokenEvent::TransferRestrictionChanged(restricted),
        );

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
            .unwrap_or(0)
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
            .unwrap_or(0)
    }

    // View function to get token price in USDC
    pub fn token_price(env: Env) -> i128 {
        let metadata = Self::get_metadata(&env);
        metadata.usdc_price
    }

    // Internal helper functions

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

    // Helper to check compliance requirements
    fn check_compliance_requirements(
        env: &Env,
        config: &ContractConfig,
        from: &Address,
        to: &Address,
    ) -> Result<(), Error> {
        // Check authorization required flag
        if config.authorization_required {
            // Check KYC status for both addresses from PERSISTENT storage
            let from_kyc = env
                .storage()
                .persistent()
                .get(&DataKey::KycVerified(from.clone()))
                .unwrap_or(false);

            let to_kyc = env
                .storage()
                .persistent()
                .get(&DataKey::KycVerified(to.clone()))
                .unwrap_or(false);

            if !from_kyc || !to_kyc {
                return Err(Error::from_contract_error(12));
            }

            // Check compliance status for both addresses from PERSISTENT storage
            let from_compliance = env
                .storage()
                .persistent()
                .get(&DataKey::ComplianceStatus(from.clone()))
                .unwrap_or(ComplianceStatus::Pending);

            let to_compliance = env
                .storage()
                .persistent()
                .get(&DataKey::ComplianceStatus(to.clone()))
                .unwrap_or(ComplianceStatus::Pending);

            if from_compliance != ComplianceStatus::Approved
                || to_compliance != ComplianceStatus::Approved
            {
                return Err(Error::from_contract_error(13));
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
            return Err(Error::from_contract_error(24)); // Self-transfer not allowed
        }

        // Get current balances from PERSISTENT storage
        let from_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);

        let to_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);

        // Check if sender has enough balance
        if from_balance < amount {
            return Err(Error::from_contract_error(14));
        }

        // Update balances in PERSISTENT storage
        let new_from_balance = from_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;
        let new_to_balance = to_balance.checked_add(amount)
            .ok_or(Error::from_contract_error(14))?;

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &new_from_balance);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &new_to_balance);

        Ok(())
    }
}

mod test;
