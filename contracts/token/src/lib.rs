#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Map, String, Vec, symbol_short,
    Error};

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

// Define compliance status enum
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ComplianceStatus {
    Pending,
    Approved,
    Rejected,
    Suspended,
}

// Define the main token contract data structure
#[contracttype]
#[derive(Clone)]
pub struct SecurityToken {
    pub metadata: TokenMetadata,
    pub balances: Map<Address, i128>,
    pub compliance_status: Map<Address, ComplianceStatus>,
    pub kyc_verified: Map<Address, bool>,
    pub admins: Vec<Address>,
    pub authorization_required: bool,
    pub authorization_revocable: bool,
    pub clawback_enabled: bool,
    pub transfer_restricted: bool,
    pub usdc_balance: i128, // Track USDC collected from purchases
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
    Purchase(Address, i128, i128), // buyer, token_amount, usdc_amount
    UsdcWithdrawn(Address, i128), // admin, amount
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
    ) -> SecurityToken {
        // Ensure the contract is only initialized once
        if env.storage().instance().has(&symbol_short!("TOKEN")) {
            panic!("Token already initialized");
        }

        // Validate parameters
        if total_supply <= 0 {
            panic!("Total supply must be positive");
        }
        if decimals > 7 {
            panic!("Decimals cannot exceed 7");
        }
        if usdc_price <= 0 {
            panic!("USDC price must be positive");
        }

        // Create token metadata
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

        // Create initial admin list
        let mut admins = Vec::new(&env);
        admins.push_back(admin.clone());
        admins.push_back(issuer.clone());

        // Initialize empty maps
        let balances = Map::new(&env);
        let compliance_status = Map::new(&env);
        let kyc_verified = Map::new(&env);

        // Assign total supply to issuer
        let mut updated_balances = balances.clone();
        updated_balances.set(issuer.clone(), total_supply);

        // Create token instance
        let token = SecurityToken {
            metadata,
            balances: updated_balances,
            compliance_status,
            kyc_verified,
            admins,
            authorization_required: true,
            authorization_revocable: true,
            clawback_enabled: true,
            transfer_restricted: true,
            usdc_balance: 0,
        };

        // Store token in contract storage
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit initialization event using tuple for topics
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::Init(token.metadata.clone()),
        );

        token
    }

    // Transfer tokens between addresses with compliance checks
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Validate amount
        if amount <= 0 {
            return Err(Error::from_contract_error(1));
        }

        // Check if transfers are currently allowed
        if token.transfer_restricted {
            // Only admins can transfer when restricted
            if !Self::is_admin(&token, &from) {
                return Err(Error::from_contract_error(2));
            }
        }

        // Check compliance requirements
        Self::check_compliance_requirements(&token, &from, &to)?;

        // Execute the transfer
        Self::execute_transfer(&env, &mut token, &from, &to, amount)?;

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit transfer event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::Transfer(from.clone(), to.clone(), amount),
        );

        Ok(())
    }

    // Set KYC verification status for an address
    pub fn set_kyc_status(
        env: Env,
        admin: Address,
        address: Address,
        verified: bool,
    ) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(3));
        }

        // Update KYC status
        token.kyc_verified.set(address.clone(), verified);

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::KycVerified(address.clone(), verified),
        );

        Ok(())
    }

    // Set compliance status for an address
    pub fn set_compliance_status(
        env: Env,
        admin: Address,
        address: Address,
        status: ComplianceStatus,
    ) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(4));
        }

        // Update compliance status
        token.compliance_status.set(address.clone(), status.clone());

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::ComplianceUpdated(address.clone(), status),
        );

        Ok(())
    }

    // Execute clawback of tokens (regulatory action)
    pub fn clawback(
        env: Env,
        admin: Address,
        from: Address,
        amount: i128,
    ) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(5));
        }

        // Check if clawback is enabled
        if !token.clawback_enabled {
            return Err(Error::from_contract_error(6));
        }

        // Get current balance
        let current_balance = token.balances.get(from.clone()).unwrap_or(0);
        if current_balance < amount {
            return Err(Error::from_contract_error(7));
        }

        // Update balances
        token.balances.set(from.clone(), current_balance - amount);

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::ClawbackExecuted(from.clone(), amount),
        );

        Ok(())
    }

    // Add an admin to the token
    pub fn add_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(8));
        }

        // Check if already an admin
        for existing_admin in token.admins.iter() {
            if &existing_admin == &new_admin {
                return Err(Error::from_contract_error(9));
            }
        }

        // Add to admin list
        token.admins.push_back(new_admin);

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        Ok(())
    }

    // Configure authorization flags
    pub fn configure_authorization(
        env: Env,
        admin: Address,
        required: bool,
        revocable: bool,
    ) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(10));
        }

        // Update authorization settings
        token.authorization_required = required;
        token.authorization_revocable = revocable;

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        // Emit event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::AuthorizationChanged(required, revocable),
        );

        Ok(())
    }
    
    // Direct purchase tokens with USDC
    pub fn purchase(
        env: Env,
        buyer: Address,
        token_amount: i128,
    ) -> Result<(), Error> {
        buyer.require_auth();
        
        // Validate amount
        if token_amount <= 0 {
            return Err(Error::from_contract_error(15));
        }
        
        // Load token from storage
        let mut token = Self::get_token(&env);
        
        // Check KYC and compliance status for buyer
        Self::check_compliance_requirements(&token, &token.metadata.issuer, &buyer)?;
        
        // Calculate USDC amount needed
        let usdc_amount = token_amount * token.metadata.usdc_price / 10i128.pow(token.metadata.decimals);
        if usdc_amount <= 0 {
            return Err(Error::from_contract_error(16));
        }
        
        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &token.metadata.usdc_token);
        
        // Transfer USDC from buyer to contract
        usdc_token_client.transfer(&buyer, &env.current_contract_address(), &usdc_amount);
        
        // Update token balances
        let issuer_balance = token.balances.get(token.metadata.issuer.clone()).unwrap_or(0);
        let buyer_balance = token.balances.get(buyer.clone()).unwrap_or(0);
        
        // Check if issuer has enough tokens
        if issuer_balance < token_amount {
            return Err(Error::from_contract_error(17));
        }
        
        // Update token balances
        token.balances.set(token.metadata.issuer.clone(), issuer_balance - token_amount);
        token.balances.set(buyer.clone(), buyer_balance + token_amount);
        
        // Update USDC balance
        token.usdc_balance += usdc_amount;
        
        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);
        
        // Emit purchase event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::Purchase(buyer.clone(), token_amount, usdc_amount),
        );
        
        Ok(())
    }
    
    // Admin function to withdraw accumulated USDC
    pub fn withdraw_usdc(
        env: Env,
        admin: Address,
        amount: i128,
    ) -> Result<(), Error> {
        admin.require_auth();
        
        // Load token from storage
        let mut token = Self::get_token(&env);
        
        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(18));
        }
        
        // Validate amount
        if amount <= 0 || amount > token.usdc_balance {
            return Err(Error::from_contract_error(19));
        }
        
        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &token.metadata.usdc_token);
        
        // Transfer USDC from contract to admin
        usdc_token_client.transfer(&env.current_contract_address(), &admin, &amount);
        
        // Update USDC balance
        token.usdc_balance -= amount;
        
        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);
        
        // Emit withdrawal event
        env.events().publish(
            (symbol_short!("VERSEPROP"),),
            SecurityTokenEvent::UsdcWithdrawn(admin.clone(), amount),
        );
        
        Ok(())
    }

    // Set transfer restriction flag
    pub fn set_transfer_restriction(
        env: Env,
        admin: Address,
        restricted: bool,
    ) -> Result<(), Error> {
        admin.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &admin) {
            return Err(Error::from_contract_error(
                11
            ));
        }

        // Update transfer restriction
        token.transfer_restricted = restricted;

        // Store updated token state
        env.storage().instance().set(&symbol_short!("TOKEN"), &token);

        Ok(())
    }

    // View function to get token metadata
    pub fn get_metadata(env: Env) -> TokenMetadata {
        let token = Self::get_token(&env);
        token.metadata
    }

    // View function to get balance
    pub fn balance(env: Env, address: Address) -> i128 {
        let token = Self::get_token(&env);
        token.balances.get(address).unwrap_or(0)
    }

    // View function to check compliance status
    pub fn check_compliance(env: Env, address: Address) -> ComplianceStatus {
        let token = Self::get_token(&env);
        token.compliance_status
            .get(address)
            .unwrap_or(ComplianceStatus::Pending)
    }

    // View function to check KYC status
    pub fn is_kyc_verified(env: Env, address: Address) -> bool {
        let token = Self::get_token(&env);
        token.kyc_verified.get(address).unwrap_or(false)
    }
    
    // View function to check accumulated USDC balance
    pub fn usdc_balance(env: Env) -> i128 {
        let token = Self::get_token(&env);
        token.usdc_balance
    }
    
    // View function to get token price in USDC
    pub fn token_price(env: Env) -> i128 {
        let token = Self::get_token(&env);
        token.metadata.usdc_price
    }

    // Internal helper functions

    // Helper to get token from storage
    fn get_token(env: &Env) -> SecurityToken {
        env.storage()
            .instance()
            .get(&symbol_short!("TOKEN"))
            .expect("Token not initialized")
    }

    // Helper to check if address is an admin
    fn is_admin(token: &SecurityToken, address: &Address) -> bool {
        for admin in token.admins.iter() {
            if &admin == address {
                return true;
            }
        }
        false
    }

    // Helper to check compliance requirements
    fn check_compliance_requirements(
        token: &SecurityToken,
        from: &Address,
        to: &Address,
    ) -> Result<(), Error> {
        // Check authorization required flag
        if token.authorization_required {
            // Check KYC status for both addresses
            let from_kyc = token.kyc_verified.get(from.clone()).unwrap_or(false);
            let to_kyc = token.kyc_verified.get(to.clone()).unwrap_or(false);

            if !from_kyc || !to_kyc {
                return Err(Error::from_contract_error(
                    12
                ));
            }

            // Check compliance status for both addresses
            let from_compliance = token
                .compliance_status
                .get(from.clone())
                .unwrap_or(ComplianceStatus::Pending);
            let to_compliance = token
                .compliance_status
                .get(to.clone())
                .unwrap_or(ComplianceStatus::Pending);

            if from_compliance != ComplianceStatus::Approved
                || to_compliance != ComplianceStatus::Approved
            {
                return Err(Error::from_contract_error(
                    13
                ));
            }
        }

        Ok(())
    }

    // Helper to execute transfer
    fn execute_transfer(
        _: &Env,
        token: &mut SecurityToken,
        from: &Address,
        to: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Get current balances
        let from_balance = token.balances.get(from.clone()).unwrap_or(0);
        let to_balance = token.balances.get(to.clone()).unwrap_or(0);

        // Check if sender has enough balance
        if from_balance < amount {
            return Err(Error::from_contract_error(14));
        }

        // Update balances
        token.balances.set(from.clone(), from_balance - amount);
        token.balances.set(to.clone(), to_balance + amount);

        Ok(())
    }
}

mod test;