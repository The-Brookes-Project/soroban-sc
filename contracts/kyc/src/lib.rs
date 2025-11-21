#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, symbol_short};

// Storage keys
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");

// Error codes
pub const ERR_ALREADY_INIT: u32 = 1;
pub const ERR_NOT_ADMIN: u32 = 2;
pub const ERR_NOT_APPROVED: u32 = 3;

// Compliance status enum
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
    KycVerified(Address),
    ComplianceStatus(Address),
}

// Event types
#[contracttype]
#[derive(Clone, Debug)]
pub enum KycEvent {
    Initialized(Address),
    KycStatusSet(Address, bool),
    ComplianceStatusSet(Address, ComplianceStatus),
}

#[contract]
pub struct KycContract;

#[contractimpl]
impl KycContract {
    /// Initialize KYC contract with admin
    pub fn initialize(
        env: Env,
        admin: Address,
    ) {
        admin.require_auth();

        // Check if already initialized
        if env.storage().instance().has(&ADMIN_KEY) {
            panic!("KYC contract already initialized");
        }

        // Store admin
        env.storage().instance().set(&ADMIN_KEY, &admin);

        // Emit event
        env.events().publish(
            (symbol_short!("init"),),
            KycEvent::Initialized(admin),
        );
    }

    /// Admin sets KYC verification status for a user
    pub fn set_kyc_status(
        env: Env,
        admin: Address,
        user: Address,
        verified: bool,
    ) {
        admin.require_auth();

        // Verify caller is admin
        let stored_admin: Address = env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("KYC contract not initialized");
        
        if admin != stored_admin {
            panic!("Not admin");
        }

        // Update KYC status in PERSISTENT storage
        env.storage()
            .persistent()
            .set(&DataKey::KycVerified(user.clone()), &verified);

        // Emit event
        env.events().publish(
            (symbol_short!("kyc_set"),),
            KycEvent::KycStatusSet(user, verified),
        );
    }

    /// Admin sets compliance status for a user
    pub fn set_compliance_status(
        env: Env,
        admin: Address,
        user: Address,
        status: ComplianceStatus,
    ) {
        admin.require_auth();

        // Verify caller is admin
        let stored_admin: Address = env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("KYC contract not initialized");
        
        if admin != stored_admin {
            panic!("Not admin");
        }

        // Update compliance status in PERSISTENT storage
        env.storage()
            .persistent()
            .set(&DataKey::ComplianceStatus(user.clone()), &status);

        // Emit event
        env.events().publish(
            (symbol_short!("comp_set"),),
            KycEvent::ComplianceStatusSet(user, status),
        );
    }

    /// Check if user is KYC verified
    pub fn is_kyc_verified(
        env: Env,
        user: Address,
    ) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::KycVerified(user))
            .unwrap_or(false)
    }

    /// Get user's compliance status
    pub fn get_compliance_status(
        env: Env,
        user: Address,
    ) -> ComplianceStatus {
        env.storage()
            .persistent()
            .get(&DataKey::ComplianceStatus(user))
            .unwrap_or(ComplianceStatus::Pending)
    }

    /// Check if user meets compliance requirements (both KYC verified and approved status)
    /// Returns Ok(()) if compliant, panics otherwise
    pub fn check_compliance(
        env: Env,
        user: Address,
    ) {
        // Check KYC verification
        let kyc_verified = Self::is_kyc_verified(env.clone(), user.clone());
        if !kyc_verified {
            panic!("User not KYC verified");
        }

        // Check compliance status
        let status = Self::get_compliance_status(env, user);
        if status != ComplianceStatus::Approved {
            panic!("User not approved for trading");
        }
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("KYC contract not initialized")
    }
}

mod test;


