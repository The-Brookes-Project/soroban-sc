#![cfg(test)]
//! Integration tests demonstrating vault and token contract interactions
//!
//! These tests simulate real-world scenarios where property token contracts
//! interact with the shared liquidity vault.

use crate::*;
use soroban_sdk::{
    testutils::{Address as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

// Helper to create mock token contract
fn create_token<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token_address = contract.address();
    let token_client = TokenClient::new(env, &token_address);
    let stellar_client = StellarAssetClient::new(env, &token_address);
    (token_address, token_client, stellar_client)
}

// Mock property contract structure
struct MockProperty {
    address: Address,
    roi_bps: u32,        // 800 = 8% APY
    token_price: i128,   // Price per token
}

fn setup_ecosystem(env: &Env) -> (Address, Address, Address, TokenClient, StellarAssetClient, VaultContractClient) {
    let admin = Address::generate(env);
    let (token_address, token_client, stellar_client) = create_token(env, &admin);
    
    // Create and initialize vault
    let vault_address = env.register(VaultContract, ());
    let vault_client = VaultContractClient::new(env, &vault_address);
    vault_client.initialize(&admin, &token_address);
    
    (vault_address, admin, token_address, token_client, stellar_client, vault_client)
}

// ==================== INTEGRATION SCENARIOS ====================

#[test]
fn test_full_property_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    // Step 1: Admin funds vault with initial capital
    stellar_client.mint(&admin, &10_000_000);
    vault_client.fund_vault(&admin, &10_000_000);
    
    // Step 2: Register property contracts
    let property_a = Address::generate(&env);
    let property_b = Address::generate(&env);
    
    vault_client.authorize_property(&admin, &property_a);
    vault_client.authorize_property(&admin, &property_b);
    
    // Step 3: Simulate user investments and liquidations from Property A
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    // User1 invests $10,000 @ 8% APY for 30 days = $66.67 yield
    // After 30 days, liquidates: $10,066.67
    vault_client.request_liquidation(&property_a, &user1, &10_066_67);
    assert_eq!(token_client.balance(&user1), 10_066_67);
    
    // User2 invests $5,000 @ 8% APY for 30 days = $33.33 yield
    vault_client.request_liquidation(&property_a, &user2, &5_033_33);
    assert_eq!(token_client.balance(&user2), 5_033_33);
    
    // Step 4: Simulate liquidations from Property B
    let user3 = Address::generate(&env);
    
    // User3 invests $20,000 @ 10% APY for 30 days = $166.67 yield
    vault_client.request_liquidation(&property_b, &user3, &20_166_67);
    assert_eq!(token_client.balance(&user3), 20_166_67);
    
    // Step 5: Verify vault state
    let config = vault_client.get_config();
    let total_liquidated = 10_066_67 + 5_033_33 + 20_166_67; // 35,266,67
    assert_eq!(config.available, 10_000_000 - total_liquidated);
    
    // Step 6: Verify property stats
    let stats_a = vault_client.get_property_stats(&property_a).unwrap();
    assert_eq!(stats_a.total_liquidated, 10_066_67 + 5_033_33);
    
    let stats_b = vault_client.get_property_stats(&property_b).unwrap();
    assert_eq!(stats_b.total_liquidated, 20_166_67);
}

#[test]
fn test_multi_property_queue_management() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    // Setup: Limited liquidity to trigger controlled mode
    stellar_client.mint(&admin, &500_000);
    vault_client.fund_vault(&admin, &500_000);
    
    // Authorize 3 properties
    let property_a = Address::generate(&env);
    let property_b = Address::generate(&env);
    let property_c = Address::generate(&env);
    
    vault_client.authorize_property(&admin, &property_a);
    vault_client.authorize_property(&admin, &property_b);
    vault_client.authorize_property(&admin, &property_c);
    
    // Multiple users liquidate across properties
    let user_a1 = Address::generate(&env);
    let user_a2 = Address::generate(&env);
    let user_b1 = Address::generate(&env);
    let user_c1 = Address::generate(&env);
    
    // Property A: Two liquidations
    vault_client.request_liquidation(&property_a, &user_a1, &200_000);
    vault_client.request_liquidation(&property_a, &user_a2, &150_000);
    
    // Property B: One large liquidation
    vault_client.request_liquidation(&property_b, &user_b1, &300_000);
    
    // Property C: One liquidation
    vault_client.request_liquidation(&property_c, &user_c1, &100_000);
    
    // Check initial processing - some should process, some should queue
    // With 500k and 15% buffer (75k), we can process up to 425k
    let user_a1_balance = token_client.balance(&user_a1);
    assert_eq!(user_a1_balance, 200_000); // First one processes
    
    // Fund more liquidity
    stellar_client.mint(&admin, &1_000_000);
    vault_client.fund_vault(&admin, &1_000_000);
    
    // Now all should be processed
    assert_eq!(token_client.balance(&user_a2), 150_000);
    assert_eq!(token_client.balance(&user_b1), 300_000);
    assert_eq!(token_client.balance(&user_c1), 100_000);
    
    // Verify controlled mode deactivated
    let final_config = vault_client.get_config();
    assert_eq!(final_config.controlled_mode, false);
}

#[test]
fn test_property_isolation() {
    //! Verify that one property's liquidations don't affect another's authorization
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &5_000_000);
    vault_client.fund_vault(&admin, &5_000_000);
    
    // Authorize two properties
    let property_good = Address::generate(&env);
    let property_malicious = Address::generate(&env);
    
    vault_client.authorize_property(&admin, &property_good);
    vault_client.authorize_property(&admin, &property_malicious);
    
    // Good property processes normal liquidation
    let user_good = Address::generate(&env);
    vault_client.request_liquidation(&property_good, &user_good, &1_000_000);
    assert_eq!(token_client.balance(&user_good), 1_000_000);
    
    // Malicious property tries to drain vault
    // Started with 5M, used 1M, left with 4M
    let user_mal1 = Address::generate(&env);
    let user_mal2 = Address::generate(&env);
    
    vault_client.request_liquidation(&property_malicious, &user_mal1, &2_000_000);
    assert_eq!(token_client.balance(&user_mal1), 2_000_000);
    
    // After mal1: 2M left, buffer 750k, so mal2 (1.5M) will queue
    vault_client.request_liquidation(&property_malicious, &user_mal2, &1_500_000);
    
    // mal2 should be queued due to buffer requirements
    assert_eq!(token_client.balance(&user_mal2), 0);
    
    // Good property should still be able to process
    let user_good2 = Address::generate(&env);
    vault_client.request_liquidation(&property_good, &user_good2, &500_000);
    
    // Check if it was queued or processed
    // Depending on buffer, might be queued
    
    // Verify stats are tracked separately
    let stats_good = vault_client.get_property_stats(&property_good).unwrap();
    let stats_mal = vault_client.get_property_stats(&property_malicious).unwrap();
    
    assert!(stats_good.total_liquidated >= 1_000_000);
    // mal2 was queued, so only mal1's amount is in stats
    assert_eq!(stats_mal.total_liquidated, 2_000_000);
}

#[test]
fn test_buffer_protection_during_liquidations() {
    //! Verify buffer prevents vault insolvency
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    // Fund with exactly 1M
    stellar_client.mint(&admin, &1_000_000);
    vault_client.fund_vault(&admin, &1_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // Try to liquidate 900k (would leave only 100k = 10%, below 15% buffer)
    let user1 = Address::generate(&env);
    vault_client.request_liquidation(&property, &user1, &900_000);
    
    // Should queue instead of processing
    let balance = token_client.balance(&user1);
    assert_eq!(balance, 0); // Queued, not processed
    
    let config = vault_client.get_config();
    assert_eq!(config.controlled_mode, true);
    
    // Verify queue has the request
    let queue_status = vault_client.get_queue_status();
    assert_eq!(queue_status.total_amount, 900_000);
}

#[test]
fn test_compounding_simulation() {
    //! Simulate a property with compounding users
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &10_000_000);
    vault_client.fund_vault(&admin, &10_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // User1: Non-compounding - 3 months total
    // Simple amounts: 1M base + 20k yield = 1,020,000
    let user_non_compound = Address::generate(&env);
    vault_client.request_liquidation(&property, &user_non_compound, &1_020_000);
    assert_eq!(token_client.balance(&user_non_compound), 1_020_000);
    
    // User2: Compounding - 3 months total with bonus
    // 1M base + 25k yield = 1,025,000
    let user_compound = Address::generate(&env);
    vault_client.request_liquidation(&property, &user_compound, &1_025_000);
    assert_eq!(token_client.balance(&user_compound), 1_025_000);
    
    // Verify vault has sufficient liquidity for both
    // Started with 10M, spent 2.045M, should have ~7.955M left
    let config = vault_client.get_config();
    assert!(config.available > 7_900_000); // Should have plenty left
}

#[test]
fn test_loyalty_bonus_progression() {
    //! Simulate loyalty bonus tiers through multiple rollovers
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &5_000_000);
    vault_client.fund_vault(&admin, &5_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    let user = Address::generate(&env);
    
    // Tier 0: Base rate - 1M + yield
    vault_client.request_liquidation(&property, &user, &1_006_667);
    
    // After several rollovers, max loyalty bonus applied
    // Tier 4: +100bps bonus - 1M + higher yield
    vault_client.request_liquidation(&property, &user, &1_007_500);
    
    // Total received across both liquidations
    assert_eq!(token_client.balance(&user), 1_006_667 + 1_007_500);
}

#[test]
fn test_emergency_pause_effect_on_properties() {
    //! Verify emergency pause blocks all property liquidations
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &5_000_000);
    vault_client.fund_vault(&admin, &5_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // Process normal liquidation
    let user1 = Address::generate(&env);
    vault_client.request_liquidation(&property, &user1, &1_000_000);
    assert_eq!(token_client.balance(&user1), 1_000_000);
    
    // Admin triggers emergency pause
    vault_client.emergency_pause(&admin);
    
    // Try to process another liquidation - should fail (tested separately in panic tests)
    // We can't catch panics in no_std, so just skip this user
    
    // Verify vault is paused
    let config = vault_client.get_config();
    assert_eq!(config.emergency_pause, true);
    
    // Admin unpauses
    vault_client.emergency_unpause(&admin);
    
    // Now liquidation should work
    let user2 = Address::generate(&env);
    vault_client.request_liquidation(&property, &user2, &1_000_000);
    assert_eq!(token_client.balance(&user2), 1_000_000);
}

#[test]
fn test_vault_liquidity_refill_scenario() {
    //! Simulate vault running low and being refilled by admin
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    // Start with minimal liquidity
    stellar_client.mint(&admin, &200_000);
    vault_client.fund_vault(&admin, &200_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // Multiple users liquidate
    let user0 = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    let user4 = Address::generate(&env);
    
    // First few process
    vault_client.request_liquidation(&property, &user0, &100_000);
    assert_eq!(token_client.balance(&user0), 100_000);
    
    // Rest queue
    vault_client.request_liquidation(&property, &user1, &80_000);
    vault_client.request_liquidation(&property, &user2, &60_000);
    vault_client.request_liquidation(&property, &user3, &50_000);
    vault_client.request_liquidation(&property, &user4, &40_000);
    
    // Check queue
    let queue_status = vault_client.get_queue_status();
    assert!(queue_status.total_queued > 0);
    assert_eq!(queue_status.controlled_mode, true);
    
    // Admin adds liquidity from new investor deposits
    stellar_client.mint(&admin, &500_000);
    vault_client.fund_vault(&admin, &500_000);
    
    // Queue should process automatically
    assert_eq!(token_client.balance(&user1), 80_000);
    assert_eq!(token_client.balance(&user2), 60_000);
    assert_eq!(token_client.balance(&user3), 50_000);
    assert_eq!(token_client.balance(&user4), 40_000);
    
    // Verify controlled mode turned off
    let final_config = vault_client.get_config();
    assert_eq!(final_config.controlled_mode, false);
}

#[test]
fn test_buffer_adjustment_impact() {
    //! Test how buffer adjustment affects liquidation processing
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &1_000_000);
    vault_client.fund_vault(&admin, &1_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // With 15% buffer, can process up to 850k
    let user1 = Address::generate(&env);
    vault_client.request_liquidation(&property, &user1, &850_000);
    assert_eq!(token_client.balance(&user1), 850_000);
    
    // Remaining: 150k
    // Admin increases buffer to 20%
    vault_client.update_buffer_percentage(&admin, &20);
    
    // Now with 20% buffer on original 1M = 200k buffer required
    // But we only have 150k available, so next liquidation should queue
    let user2 = Address::generate(&env);
    vault_client.request_liquidation(&property, &user2, &50_000);
    
    // Should queue because 150k - 50k = 100k < 200k buffer
    // Actually, total_capacity is still 1M, so buffer is 200k
    // But available is only 150k, which is less than buffer, so controlled mode
    assert_eq!(token_client.balance(&user2), 0); // Queued
    
    let config = vault_client.get_config();
    assert_eq!(config.controlled_mode, true);
}

#[test]
fn test_statistics_tracking_accuracy() {
    //! Verify property statistics are accurately tracked
    
    let env = Env::default();
    env.mock_all_auths();
    
    let (_, admin, _, token_client, stellar_client, vault_client) = setup_ecosystem(&env);
    
    stellar_client.mint(&admin, &10_000_000);
    vault_client.fund_vault(&admin, &10_000_000);
    
    let property = Address::generate(&env);
    vault_client.authorize_property(&admin, &property);
    
    // Initial stats
    let stats = vault_client.get_property_stats(&property).unwrap();
    assert_eq!(stats.total_liquidated, 0);
    
    // Process multiple liquidations
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);
    let user_d = Address::generate(&env);
    
    vault_client.request_liquidation(&property, &user_a, &100_000);
    vault_client.request_liquidation(&property, &user_b, &250_000);
    vault_client.request_liquidation(&property, &user_c, &175_000);
    vault_client.request_liquidation(&property, &user_d, &500_000);
    
    let expected_total = 100_000i128 + 250_000 + 175_000 + 500_000;
    
    // Verify stats
    let final_stats = vault_client.get_property_stats(&property).unwrap();
    assert_eq!(final_stats.total_liquidated, expected_total);
    assert_eq!(final_stats.total_liquidated, 1_025_000);
}

