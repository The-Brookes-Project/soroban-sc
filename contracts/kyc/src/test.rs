#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Verify admin
    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "KYC contract already initialized")]
fn test_initialize_twice() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Try to initialize again - should panic
    client.initialize(&admin);
}

#[test]
fn test_set_and_get_kyc_status() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // User should not be verified initially
    assert_eq!(client.is_kyc_verified(&user), false);

    // Set KYC status to verified
    client.set_kyc_status(&admin, &user, &true);

    // Check verified
    assert_eq!(client.is_kyc_verified(&user), true);

    // Set to not verified
    client.set_kyc_status(&admin, &user, &false);

    // Check not verified
    assert_eq!(client.is_kyc_verified(&user), false);
}

#[test]
#[should_panic(expected = "Not admin")]
fn test_set_kyc_status_not_admin() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Try to set KYC as non-admin - should panic
    client.set_kyc_status(&non_admin, &user, &true);
}

#[test]
fn test_set_and_get_compliance_status() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // User should be Pending initially
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Pending);

    // Set to Approved
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Approved);
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Approved);

    // Set to Rejected
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Rejected);
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Rejected);

    // Set to Suspended
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Suspended);
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Suspended);

    // Set back to Pending
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Pending);
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Pending);
}

#[test]
#[should_panic(expected = "Not admin")]
fn test_set_compliance_status_not_admin() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Try to set compliance as non-admin - should panic
    client.set_compliance_status(&non_admin, &user, &ComplianceStatus::Approved);
}

#[test]
fn test_check_compliance_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set both KYC and compliance to approved
    client.set_kyc_status(&admin, &user, &true);
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Approved);

    // Should not panic
    client.check_compliance(&user);
}

#[test]
#[should_panic(expected = "User not KYC verified")]
fn test_check_compliance_not_kyc_verified() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set compliance but not KYC
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Approved);

    // Should panic - not KYC verified
    client.check_compliance(&user);
}

#[test]
#[should_panic(expected = "User not approved for trading")]
fn test_check_compliance_not_approved() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set KYC but not compliance to Approved
    client.set_kyc_status(&admin, &user, &true);
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Pending);

    // Should panic - not approved
    client.check_compliance(&user);
}

#[test]
#[should_panic(expected = "User not approved for trading")]
fn test_check_compliance_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set both but status is Rejected
    client.set_kyc_status(&admin, &user, &true);
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Rejected);

    // Should panic - rejected
    client.check_compliance(&user);
}

#[test]
#[should_panic(expected = "User not approved for trading")]
fn test_check_compliance_suspended() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set both but status is Suspended
    client.set_kyc_status(&admin, &user, &true);
    client.set_compliance_status(&admin, &user, &ComplianceStatus::Suspended);

    // Should panic - suspended
    client.check_compliance(&user);
}

#[test]
fn test_multiple_users() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(KycContract, ());
    let client = KycContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    // Initialize
    client.initialize(&admin);

    // Set different statuses for different users
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    client.set_kyc_status(&admin, &user2, &true);
    client.set_compliance_status(&admin, &user2, &ComplianceStatus::Rejected);

    client.set_kyc_status(&admin, &user3, &false);
    client.set_compliance_status(&admin, &user3, &ComplianceStatus::Pending);

    // Verify each user's status
    assert_eq!(client.is_kyc_verified(&user1), true);
    assert_eq!(client.get_compliance_status(&user1), ComplianceStatus::Approved);

    assert_eq!(client.is_kyc_verified(&user2), true);
    assert_eq!(client.get_compliance_status(&user2), ComplianceStatus::Rejected);

    assert_eq!(client.is_kyc_verified(&user3), false);
    assert_eq!(client.get_compliance_status(&user3), ComplianceStatus::Pending);

    // Check compliance only passes for user1
    client.check_compliance(&user1); // Should pass
}

