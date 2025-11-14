#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, token};

// Helper function to setup test environment
fn setup_test_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let vault_address = Address::generate(&env);
    let kyc_address = Address::generate(&env);
    let usdc_address = Address::generate(&env);
    
    (env, admin, user, vault_address, kyc_address, usdc_address)
}

fn setup_property_contract(env: &Env, admin: &Address, vault: &Address, kyc: &Address, usdc: &Address) -> Address {
    let contract_id = env.register(PropertyContract, ());
    let client = PropertyContractClient::new(env, &contract_id);
    
    client.initialize(
        admin,
        &String::from_str(env, "Test Property"),
        &String::from_str(env, "TPROP"),
        &7,
        &1_000_000_0000000,  // 1 million tokens
        &100_0000000,  // $100 per token
        vault,
        kyc,
        usdc,
    );
    
    contract_id
}

#[test]
fn test_initialize() {
    let (env, admin, _, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // Check metadata
    let metadata = client.get_metadata();
    assert_eq!(metadata.name, String::from_str(&env, "Test Property"));
    assert_eq!(metadata.symbol, String::from_str(&env, "TPROP"));
    assert_eq!(metadata.decimals, 7);
    
    // Check admin
    assert_eq!(client.get_admin(), admin);
    
    // Check total active tokens
    assert_eq!(client.total_active_tokens(), 0);
}

#[test]
#[should_panic(expected = "Property contract already initialized")]
fn test_initialize_twice() {
    let (env, admin, _, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = env.register(PropertyContract, ());
    let client = PropertyContractClient::new(&env, &contract_id);
    
    client.initialize(
        &admin,
        &String::from_str(&env, "Test Property"),
        &String::from_str(&env, "TPROP"),
        &7,
        &1_000_000_0000000,
        &100_0000000,
        &vault,
        &kyc,
        &usdc,
    );
    
    // Try to initialize again - should panic
    client.initialize(
        &admin,
        &String::from_str(&env, "Test Property"),
        &String::from_str(&env, "TPROP"),
        &7,
        &1_000_000_0000000,
        &100_0000000,
        &vault,
        &kyc,
        &usdc,
    );
}

#[test]
fn test_update_roi_config() {
    let (env, admin, _, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // Update ROI config
    client.update_roi_config(&admin, &1000, &300, &25, &50_000_0000000);
    
    // Check updated config
    let roi_config = client.get_roi_config();
    assert_eq!(roi_config.annual_rate_bps, 1000);  // 10% APY
    assert_eq!(roi_config.compounding_bonus_bps, 300);  // +3% bonus
    assert_eq!(roi_config.loyalty_bonus_bps, 25);
    assert_eq!(roi_config.cash_flow_monthly, 50_000_0000000);
}

#[test]
fn test_can_take_action() {
    let (env, admin, user, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // User has no position initially
    assert_eq!(client.can_take_action(&user), false);
}

#[test]
fn test_is_in_grace_period() {
    let (env, admin, user, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // User has no position initially
    assert_eq!(client.is_in_grace_period(&user), false);
}

#[test]
fn test_can_admin_rollover() {
    let (env, admin, user, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // User has no position initially
    assert_eq!(client.can_admin_rollover(&user), false);
}

#[test]
fn test_get_user_position_none() {
    let (env, admin, user, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    // User has no position initially
    assert_eq!(client.get_user_position(&user).is_none(), true);
}

#[test]
fn test_total_active_tokens_initial() {
    let (env, admin, _, vault, kyc, usdc) = setup_test_env();
    
    let contract_id = setup_property_contract(&env, &admin, &vault, &kyc, &usdc);
    let client = PropertyContractClient::new(&env, &contract_id);
    
    assert_eq!(client.total_active_tokens(), 0);
}

// Note: Full integration tests with actual KYC, vault contracts, and token purchases
// will be in the integration test file since they require cross-contract calls

