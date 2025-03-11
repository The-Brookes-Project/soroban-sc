#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Map, String, Vec,
                  symbol_short, Error, Symbol};

const TOKEN_KEY: Symbol = symbol_short!("VERSEPROP");

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
    ) -> SecurityToken {
        // Get the deployer's address (the contract ID before initialization)
        let deployer = env.current_contract_address();

        // Require authorization from the deployer
        deployer.require_auth();

        // Ensure the contract is only initialized once
        if env.storage().instance().has(&TOKEN_KEY) {
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
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit initialization event using tuple for topics
        env.events().publish(
            (TOKEN_KEY,),
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
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit transfer event
        env.events().publish(
            (TOKEN_KEY,),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(3));
        }

        // Update KYC status
        token.kyc_verified.set(address.clone(), verified);

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit event
        env.events().publish(
            (TOKEN_KEY,),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(4));
        }

        // Update compliance status
        token.compliance_status.set(address.clone(), status.clone());

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit event
        env.events().publish(
            (TOKEN_KEY,),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(5));
        }

        // Check if clawback is enabled
        if !token.clawback_enabled {
            return Err(Error::from_contract_error(6));
        }

        // Get current balance
        let current_balance = token.balances.try_get(from.clone())
            .map_err(|_| Error::from_contract_error(7))?
            .unwrap_or(0);

        if current_balance < amount {
            return Err(Error::from_contract_error(7));
        }

        // Update balances
        let new_balance = current_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;
        token.balances.set(from.clone(), new_balance);

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit event
        env.events().publish(
            (TOKEN_KEY,),
            SecurityTokenEvent::ClawbackExecuted(from.clone(), amount),
        );

        Ok(())
    }

    // Add an admin to the token
    pub fn add_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), Error> {
        caller.require_auth();

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(8));
        }

        // Check if already an admin
        for existing_admin in token.admins.iter() {
            if &existing_admin == &new_admin {
                return Err(Error::from_contract_error(9));
            }
        }

        // Add to admin list
        token.admins.push_back(new_admin.clone());

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit admin added event
        env.events().publish(
            (TOKEN_KEY,),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(10));
        }

        // Update authorization settings
        token.authorization_required = required;
        token.authorization_revocable = revocable;

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit event
        env.events().publish(
            (TOKEN_KEY,),
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
        let decimals_pow = 10i128.checked_pow(token.metadata.decimals)
            .ok_or(Error::from_contract_error(16))?;

        let usdc_amount = token_amount.checked_mul(token.metadata.usdc_price)
            .ok_or(Error::from_contract_error(16))?
            .checked_div(decimals_pow)
            .ok_or(Error::from_contract_error(16))?;

        if usdc_amount <= 0 {
            return Err(Error::from_contract_error(16));
        }

        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &token.metadata.usdc_token);

        // Transfer USDC from buyer to contract
        usdc_token_client.transfer(&buyer, &env.current_contract_address(), &usdc_amount);

        // Update token balances
        let issuer_balance = token.balances.try_get(token.metadata.issuer.clone())
            .map_err(|_| Error::from_contract_error(17))?
            .unwrap_or(0);

        let buyer_balance = token.balances.try_get(buyer.clone())
            .map_err(|_| Error::from_contract_error(17))?
            .unwrap_or(0);

        // Check if issuer has enough tokens
        if issuer_balance < token_amount {
            return Err(Error::from_contract_error(17));
        }

        // Update token balances
        let new_issuer_balance = issuer_balance.checked_sub(token_amount)
            .ok_or(Error::from_contract_error(14))?;
        let new_buyer_balance = buyer_balance.checked_add(token_amount)
            .ok_or(Error::from_contract_error(14))?;

        token.balances.set(token.metadata.issuer.clone(), new_issuer_balance);
        token.balances.set(buyer.clone(), new_buyer_balance);

        // Update USDC balance
        token.usdc_balance = token.usdc_balance.checked_add(usdc_amount)
            .ok_or(Error::from_contract_error(14))?;

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit purchase event
        env.events().publish(
            (TOKEN_KEY,),
            SecurityTokenEvent::Purchase(buyer.clone(), token_amount, usdc_amount),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(18));
        }

        // Validate amount
        if amount <= 0 || amount > token.usdc_balance {
            return Err(Error::from_contract_error(19));
        }

        // Get USDC token client
        let usdc_token_client = token::Client::new(&env, &token.metadata.usdc_token);

        // Transfer USDC from contract to admin
        usdc_token_client.transfer(&env.current_contract_address(), &caller, &amount);

        // Update USDC balance
        token.usdc_balance = token.usdc_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit withdrawal event
        env.events().publish(
            (TOKEN_KEY,),
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

        // Load token from storage
        let mut token = Self::get_token(&env);

        // Check if caller is admin
        if !Self::is_admin(&token, &caller) {
            return Err(Error::from_contract_error(
                11
            ));
        }

        // Update transfer restriction
        token.transfer_restricted = restricted;

        // Store updated token state
        env.storage().instance().set(&TOKEN_KEY, &token);

        // Emit transfer restriction changed event
        env.events().publish(
            (TOKEN_KEY,),
            SecurityTokenEvent::TransferRestrictionChanged(restricted),
        );

        Ok(())
    }

    // View function to get token metadata
    pub fn get_metadata(env: Env) -> TokenMetadata {
        let token = Self::get_token(&env);
        token.metadata
    }

    // View function to get balance
    pub fn balance(env: Env, address: Address) -> Result<i128, Error> {
        let token = Self::get_token(&env);
        token.balances.try_get(address)
            .map(|opt| opt.unwrap_or(0))
            .map_err(Error::from)
    }

    // View function to check compliance status
    pub fn check_compliance(env: Env, address: Address) -> Result<ComplianceStatus, Error> {
        let token = Self::get_token(&env);
        token.compliance_status.try_get(address)
            .map(|opt| opt.unwrap_or(ComplianceStatus::Pending))
            .map_err(Error::from)
    }

    // View function to check KYC status
    pub fn is_kyc_verified(env: Env, address: Address) -> Result<bool, Error> {
        let token = Self::get_token(&env);
        token.kyc_verified.try_get(address)
            .map(|opt| opt.unwrap_or(false))
            .map_err(Error::from)
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
            .get(&TOKEN_KEY)
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
            let from_kyc = token.kyc_verified.try_get(from.clone())
                .map_err(|_| Error::from_contract_error(12))?
                .unwrap_or(false);

            let to_kyc = token.kyc_verified.try_get(to.clone())
                .map_err(|_| Error::from_contract_error(12))?
                .unwrap_or(false);

            if !from_kyc || !to_kyc {
                return Err(Error::from_contract_error(12));
            }

            // Check compliance status for both addresses
            let from_compliance = token.compliance_status.try_get(from.clone())
                .map_err(|_| Error::from_contract_error(13))?
                .unwrap_or(ComplianceStatus::Pending);

            let to_compliance = token.compliance_status.try_get(to.clone())
                .map_err(|_| Error::from_contract_error(13))?
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
        _: &Env,
        token: &mut SecurityToken,
        from: &Address,
        to: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Get current balances
        let from_balance = token.balances.try_get(from.clone())
            .map_err(|_| Error::from_contract_error(14))?
            .unwrap_or(0);

        let to_balance = token.balances.try_get(to.clone())
            .map_err(|_| Error::from_contract_error(14))?
            .unwrap_or(0);

        // Check if sender has enough balance
        if from_balance < amount {
            return Err(Error::from_contract_error(14));
        }

        // Update balances
        let new_from_balance = from_balance.checked_sub(amount)
            .ok_or(Error::from_contract_error(14))?;
        let new_to_balance = to_balance.checked_add(amount)
            .ok_or(Error::from_contract_error(14))?;

        token.balances.set(from.clone(), new_from_balance);
        token.balances.set(to.clone(), new_to_balance);

        Ok(())
    }
}

mod test;