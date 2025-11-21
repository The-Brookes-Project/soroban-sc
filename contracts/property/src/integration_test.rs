#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger, LedgerInfo}, Address, Env, String, token};

// Import the real contracts
use crate::{KycContractClient, VaultContractClient};

// Helper to create USDC token
fn create_usdc_token(env: &Env, admin: &Address) -> Address {
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    token_id.address()
}

#[test]
fn test_full_purchase_rollover_liquidation_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup USDC token
    let usdc_id = create_usdc_token(&env, &admin);
    let usdc_client = token::StellarAssetClient::new(&env, &usdc_id);
    
    // Mint USDC to user for purchase (10,000 USDC)
    usdc_client.mint(&user, &10_000_0000000);
    
    // Setup KYC contract
    let kyc_id = env.register(crate::kyc_contract::WASM, ());
    let kyc_client = KycContractClient::new(&env, &kyc_id);
    kyc_client.initialize(&admin);
    
    // Approve user KYC
    kyc_client.set_kyc_status(&admin, &user, &true);
    kyc_client.set_compliance_status(&admin, &user, &crate::kyc_contract::ComplianceStatus::Approved);
    
    // Setup Vault contract
    let vault_id = env.register(crate::vault_contract::WASM, ());
    let vault_client = VaultContractClient::new(&env, &vault_id);
    vault_client.initialize(&admin, &usdc_id);
    
    // Fund vault with liquidity (100,000 USDC)
    usdc_client.mint(&admin, &100_000_0000000);
    vault_client.fund_vault(&admin, &100_000_0000000);
    
    // Setup Property contract
    let property_id = env.register(PropertyContract, ());
    let property_client = PropertyContractClient::new(&env, &property_id);
    property_client.initialize(
        &admin,
        &String::from_str(&env, "Test Property"),
        &String::from_str(&env, "TPROP"),
        &7,
        &1_000_000_0000000,
        &100_0000000,  // $100 per token
        &vault_id,
        &kyc_id,
        &usdc_id,
    );
    
    // Authorize property in vault
    vault_client.authorize_property(&admin, &property_id);
    
    // Update ROI config
    property_client.update_roi_config(&admin, &800, &200, &25, &10_000_0000000);
    
    // 1. USER PURCHASES TOKENS
    // Approve property contract to spend USDC (using reasonable expiration ledger)
    let usdc_token_client = token::Client::new(&env, &usdc_id);
    let expiration_ledger = env.ledger().sequence() + 1000;
    usdc_token_client.approve(&user, &property_id, &10_000_0000000, &expiration_ledger);
    property_client.purchase_tokens(&user, &100_0000000, &true);  // 100 tokens, $10,000, compounding enabled
    
    // Verify position created
    let position = property_client.get_user_position(&user).unwrap();
    assert_eq!(position.tokens, 100_0000000);
    // Cost = (100 tokens * 100 USDC/token) properly scaled = 10,000 USDC
    assert_eq!(position.initial_investment, 10_000_0000000);
    assert_eq!(position.current_principal, 10_000_0000000);
    assert_eq!(position.compounding_enabled, true);
    assert_eq!(position.consecutive_rollovers, 0);
    assert_eq!(position.loyalty_tier, 0);
    
    // Check user can't take action yet (epoch not complete)
    assert_eq!(property_client.can_take_action(&user), false);
    
    // 2. FAST FORWARD 30 DAYS
    env.ledger().set(LedgerInfo {
        timestamp: env.ledger().timestamp() + 2_592_000,  // 30 days
        protocol_version: 23,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10000,
    });
    
    // Now user can take action
    assert_eq!(property_client.can_take_action(&user), true);
    
    // Preview yield
    let yield_preview = property_client.preview_yield(&user);
    // With cost = 10,000 USDC and integer division:
    // Base yield: 10,000 * (800/12) / 10,000 = 10,000 * 66 / 10,000 = 66 USDC
    // Compounding bonus: 10,000 * (200/12) / 10,000 = 10,000 * 16 / 10,000 = 16 USDC
    // Total: 82 USDC (integer math truncates 800/12=66.666 to 66, and 200/12=16.666 to 16)
    assert_eq!(yield_preview.total_yield, 82_0000000);
    assert_eq!(yield_preview.days_elapsed, 30);
    assert_eq!(yield_preview.days_remaining, 0);
    
    // 3. USER ROLLS OVER
    property_client.rollover_position(&user);
    
    // Verify position updated
    let position = property_client.get_user_position(&user).unwrap();
    assert_eq!(position.consecutive_rollovers, 1);
    assert_eq!(position.loyalty_tier, 1);
    // Principal should include yield since compounding is enabled
    // Initial: 10,000 USDC, after yield: 10,082 USDC (with integer math)
    assert!(position.current_principal > 10_000_0000000);
    assert_eq!(position.current_principal, 10_082_0000000);
    
    // 4. FAST FORWARD ANOTHER 30 DAYS
    env.ledger().set(LedgerInfo {
        timestamp: env.ledger().timestamp() + 2_592_000,  // Another 30 days
        protocol_version: 23,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10000,
    });
    
    // Preview yield with loyalty bonus
    let yield_preview = property_client.preview_yield(&user);
    // Second epoch with loyalty tier 1 (25 bps), principal = 10,082 USDC (100820000000):
    // Base: 100820000000 * 66 / 10,000 = 665412000
    // Compounding: 100820000000 * 16 / 10,000 = 161312000
    // Loyalty: 100820000000 * 2 / 10,000 = 20164000
    // Total: 846888000 (84.6888 USDC)
    assert!(yield_preview.loyalty_bonus > 0);
    assert_eq!(yield_preview.total_yield, 846888000);
    
    // 5. USER LIQUIDATES
    let user_balance_before = usdc_client.balance(&user);
    property_client.liquidate_position(&user);
    
    // Verify position removed
    assert!(property_client.get_user_position(&user).is_none());
    
    // Verify user received payout from vault
    let user_balance_after = usdc_client.balance(&user);
    assert!(user_balance_after > user_balance_before);
    // User should have received principal + yield
    // Epoch 1: 10,000 + 82 = 10,082 USDC (100820000000)
    // Epoch 2: 10,082 + 84.6888 = 10,166.6888 USDC (101666888000)
    assert_eq!(user_balance_after, 101666888000);
}

#[test]
fn test_admin_rollover_after_grace_period() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup USDC token
    let usdc_id = create_usdc_token(&env, &admin);
    let usdc_client = token::StellarAssetClient::new(&env, &usdc_id);
    
    // Mint USDC to user
    usdc_client.mint(&user, &10_000_0000000);
    
    // Setup KYC contract
    let kyc_id = env.register(crate::kyc_contract::WASM, ());
    let kyc_client = KycContractClient::new(&env, &kyc_id);
    kyc_client.initialize(&admin);
    kyc_client.set_kyc_status(&admin, &user, &true);
    kyc_client.set_compliance_status(&admin, &user, &crate::kyc_contract::ComplianceStatus::Approved);
    
    // Setup Vault contract
    let vault_id = env.register(crate::vault_contract::WASM, ());
    let vault_client = VaultContractClient::new(&env, &vault_id);
    vault_client.initialize(&admin, &usdc_id);
    usdc_client.mint(&admin, &100_000_0000000);
    vault_client.fund_vault(&admin, &100_000_0000000);
    
    // Setup Property contract
    let property_id = env.register(PropertyContract, ());
    let property_client = PropertyContractClient::new(&env, &property_id);
    property_client.initialize(
        &admin,
        &String::from_str(&env, "Test Property"),
        &String::from_str(&env, "TPROP"),
        &7,
        &1_000_000_0000000,
        &100_0000000,
        &vault_id,
        &kyc_id,
        &usdc_id,
    );
    
    vault_client.authorize_property(&admin, &property_id);
    
    // User purchases
    // Approve property contract to spend USDC (using reasonable expiration ledger)
    let usdc_token_client = token::Client::new(&env, &usdc_id);
    let expiration_ledger = env.ledger().sequence() + 1000;
    usdc_token_client.approve(&user, &property_id, &10_000_0000000, &expiration_ledger);
    property_client.purchase_tokens(&user, &100_0000000, &false);  // No compounding
    
    // Fast forward 30 days + 24 hours (grace period)
    env.ledger().set(LedgerInfo {
        timestamp: env.ledger().timestamp() + 2_592_000 + 86_400,  // 30 days + 24 hours
        protocol_version: 23,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10000,
    });
    
    // Check admin can rollover
    assert_eq!(property_client.can_admin_rollover(&user), true);
    
    // Admin rolls over user's position
    property_client.admin_rollover_position(&admin, &user);
    
    // Verify position updated
    let position = property_client.get_user_position(&user).unwrap();
    assert_eq!(position.consecutive_rollovers, 1);
    assert_eq!(position.loyalty_tier, 1);
    // Principal should NOT include yield since compounding is disabled
    assert_eq!(position.current_principal, 10_000_0000000);
    // But total yield earned should be tracked
    assert!(position.total_yield_earned > 0);
}

#[test]
fn test_loyalty_tier_progression() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup all contracts
    let usdc_id = create_usdc_token(&env, &admin);
    let usdc_client = token::StellarAssetClient::new(&env, &usdc_id);
    usdc_client.mint(&user, &10_000_0000000);
    
    let kyc_id = env.register(crate::kyc_contract::WASM, ());
    let kyc_client = KycContractClient::new(&env, &kyc_id);
    kyc_client.initialize(&admin);
    kyc_client.set_kyc_status(&admin, &user, &true);
    kyc_client.set_compliance_status(&admin, &user, &crate::kyc_contract::ComplianceStatus::Approved);
    
    let vault_id = env.register(crate::vault_contract::WASM, ());
    let vault_client = VaultContractClient::new(&env, &vault_id);
    vault_client.initialize(&admin, &usdc_id);
    usdc_client.mint(&admin, &100_000_0000000);
    vault_client.fund_vault(&admin, &100_000_0000000);
    
    let property_id = env.register(PropertyContract, ());
    let property_client = PropertyContractClient::new(&env, &property_id);
    property_client.initialize(
        &admin,
        &String::from_str(&env, "Test Property"),
        &String::from_str(&env, "TPROP"),
        &7,
        &1_000_000_0000000,
        &100_0000000,
        &vault_id,
        &kyc_id,
        &usdc_id,
    );
    vault_client.authorize_property(&admin, &property_id);
    
    // User purchases
    // Approve property contract to spend USDC (using reasonable expiration ledger)
    let usdc_token_client = token::Client::new(&env, &usdc_id);
    let expiration_ledger = env.ledger().sequence() + 1000;
    usdc_token_client.approve(&user, &property_id, &10_000_0000000, &expiration_ledger);
    property_client.purchase_tokens(&user, &100_0000000, &true);
    
    // Rollover 5 times to test max tier (4)
    for i in 0..5 {
        // Fast forward 30 days
        env.ledger().set(LedgerInfo {
            timestamp: env.ledger().timestamp() + 2_592_000,
            protocol_version: 23,
            sequence_number: env.ledger().sequence(),
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1000,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 10000,
        });
        
        property_client.rollover_position(&user);
        
        let position = property_client.get_user_position(&user).unwrap();
        assert_eq!(position.consecutive_rollovers, i + 1);
        
        // Tier should max out at 4
        let expected_tier = if i + 1 > 4 { 4 } else { i + 1 };
        assert_eq!(position.loyalty_tier, expected_tier);
    }
    
    // Verify max tier is 4
    let position = property_client.get_user_position(&user).unwrap();
    assert_eq!(position.loyalty_tier, 4);
}


