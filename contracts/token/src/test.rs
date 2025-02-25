#![cfg(test)]
use super::*;
use soroban_sdk::Env;
use soroban_sdk::testutils::Address as SorobanAddress;

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
        )
    });

    assert_eq!(client.get_metadata().name, String::from_str(&env, "Security Token"));
    assert_eq!(client.get_metadata().symbol, String::from_str(&env, "SCTY"));
    assert_eq!(client.get_metadata().decimals, 6);
    assert_eq!(client.get_metadata().total_supply, 1_000_000_000_000);
    assert_eq!(client.get_metadata().issuer, issuer);
    assert_eq!(client.get_metadata().home_domain, String::from_str(&env, "example.com"));
    assert_eq!(client.balance(&issuer), 1_000_000_000_000);
}

#[test]
fn test_transfer_with_compliance() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Create client
    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
        )
    });

    // Mock authentication for all calls
    env.mock_all_auths();

    // Set KYC status for users
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_kyc_status(&admin, &user2, &true);

    // Set compliance status for users
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user2, &ComplianceStatus::Approved);

    client.set_transfer_restriction(&admin, &false);

    // Transfer from issuer to user1
    client.transfer(&issuer, &user1, &100_000);

    // Check balances
    let user1_balance = client.balance(&user1);
    assert_eq!(user1_balance, 100_000);

    // Transfer from user1 to user2
    client.transfer(&user1, &user2, &50_000);

    // Check updated balances
    let user1_balance = client.balance(&user1);
    let user2_balance = client.balance(&user2);
    assert_eq!(user1_balance, 50_000);
    assert_eq!(user2_balance, 50_000);
}

#[test]
fn test_clawback() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
        )
    });

    // Mock authentication for all calls
    env.mock_all_auths();

    // Now these calls will work with auth mocked
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Transfer tokens to user1
    client.transfer(&issuer, &user1, &100_000);

    let initial_balance = client.balance(&user1);
    assert_eq!(initial_balance, 100_000);

    // Execute clawback
    client.clawback(&admin, &user1, &25_000);

    // Verify balance after clawback
    let final_balance = client.balance(&user1);
    assert_eq!(final_balance, 75_000);
}