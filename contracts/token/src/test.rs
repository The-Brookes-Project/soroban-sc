#![cfg(test)]
use super::*;
use soroban_sdk::{
    Env,
    testutils::{Address as SorobanAddress},
    token,
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let sac = e.register_stellar_asset_contract_v2(admin.clone());
    (
        token::Client::new(e, &sac.address()),
        token::StellarAssetClient::new(e, &sac.address()),
    )
}

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

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
            100_000, // 0.1 USDC per token
            usdc_token_client.address
        )
    });

    assert_eq!(client.get_metadata().name, String::from_str(&env, "Security Token"));
    assert_eq!(client.get_metadata().symbol, String::from_str(&env, "SCTY"));
    assert_eq!(client.get_metadata().decimals, 6);
    assert_eq!(client.get_metadata().total_supply, 1_000_000_000_000);
    assert_eq!(client.get_metadata().issuer, issuer);
    assert_eq!(client.get_metadata().home_domain, String::from_str(&env, "example.com"));
    assert_eq!(client.token_price(), 100_000);
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

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

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
            100_000, // 0.1 USDC per token
            usdc_token_client.address
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

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

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
            100_000, // 0.1 USDC per token
            usdc_token_client.address
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

#[test]
fn test_purchase_and_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;
    let usdc_token_admin_client = usdc_token.1;

    let current_ledger = env.ledger().sequence();
    let expiration_ledger = current_ledger + 100;

    usdc_token_client.approve(
        &buyer,
        &contract_id,       // e.g. your SecurityToken contract's address
        &1_000_000_000i128, // allowance
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

    // Create security token client
    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with price of 0.1 USDC per token
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
            100_000, // 0.1 USDC per token
            usdc_token_client.address.clone()
        )
    });

    // Set KYC and compliance status
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Initial balances
    let initial_buyer_token_balance = client.balance(&buyer);
    let initial_issuer_token_balance = client.balance(&issuer);
    let initial_buyer_usdc_balance = usdc_token_client.balance(&buyer);
    assert_eq!(initial_buyer_token_balance, 0);
    assert_eq!(initial_issuer_token_balance, 1_000_000_000_000);
    assert_eq!(initial_buyer_usdc_balance, 1_000_000_000);

    // Buyer purchases 500,000 tokens for 50,000,000 (0.1 USDC per token)
    let purchase_amount = 500_000_000;
    client.purchase(&buyer, &purchase_amount);

    // Check token balances after purchase
    let buyer_token_balance = client.balance(&buyer);
    let issuer_token_balance = client.balance(&issuer);
    let buyer_usdc_balance = usdc_token_client.balance(&buyer);
    let contract_usdc_balance = client.usdc_balance();

    assert_eq!(buyer_token_balance, 500_000_000);
    assert_eq!(issuer_token_balance, 999_500_000_000);
    assert_eq!(buyer_usdc_balance, 950_000_000); // 1B - 50M
    assert_eq!(contract_usdc_balance, 50_000_000);

    // Admin withdraws USDC
    client.withdraw_usdc(&admin, &30_000_000);

    // Check balances after withdrawal
    let admin_usdc_balance = usdc_token_client.balance(&admin);
    let updated_contract_usdc_balance = client.usdc_balance();

    assert_eq!(admin_usdc_balance, 30_000_000);
    assert_eq!(updated_contract_usdc_balance, 20_000_000); // 50M - 30M
}

// ===== Failure Test Cases =====

#[test]
#[should_panic]
fn test_purchase_insufficient_usdc_balance() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Setup test USDC token contract
    let (usdc_token_client, usdc_token_admin_client) = create_token_contract(&env, &admin);

    let current_ledger = env.ledger().sequence();
    let expiration_ledger = current_ledger + 100;
    usdc_token_client.approve(
        &buyer,
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );
    // Mint an insufficient USDC balance to buyer (e.g., 10_000_000)
    usdc_token_admin_client.mint(&buyer, &10_000_000);

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with a token price of 100_000 per token
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
            100_000,
            usdc_token_client.address.clone()
        )
    });

    // Set KYC and compliance status for issuer and buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Attempt purchase that exceeds buyer's available USDC balance.
    // For example, if buyer can only afford 100 tokens, purchasing 200 tokens should fail.
    let purchase_amount = 200_000_000;
    client.purchase(&buyer, &purchase_amount);
}

#[test]
#[should_panic]
fn test_transfer_without_kyc() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let (usdc_token_client, _) = create_token_contract(&env, &admin);

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
            100_000,
            usdc_token_client.address.clone()
        )
    });

    env.mock_all_auths();

    // Set KYC only for issuer (user1 remains unset)
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Attempt transfer from issuer to user1 should fail due to missing KYC for user1.
    client.transfer(&issuer, &user1, &100_000);
}

#[test]
#[should_panic]
fn test_transfer_restricted_fail() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let (usdc_token_client, _) = create_token_contract(&env, &admin);

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
            100_000, // USDC price per token
            usdc_token_client.address.clone()
        )
    });

    env.mock_all_auths();

    // Set KYC and compliance for issuer and user1
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Explicitly enable transfer restrictions (token initializes with this true by default)
    client.set_transfer_restriction(&admin, &true);

    // Transfer tokens from issuer (an admin) to user1 so that user1 has tokens.
    client.transfer(&issuer, &user1, &100_000);

    // Now attempt a transfer from user1 (non-admin).
    // This should fail and panic, since user1 is not allowed to transfer while transfers are restricted.
    client.transfer(&user1, &issuer, &50_000);
}

#[test]
#[should_panic]
fn test_clawback_exceeds_balance() {
    let env = Env::default();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let (usdc_token_client, _) = create_token_contract(&env, &admin);

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
            100_000,
            usdc_token_client.address.clone()
        )
    });

    env.mock_all_auths();

    // Set KYC and compliance for issuer and user1
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Transfer a small amount to user1
    client.transfer(&issuer, &user1, &50_000);

    // Attempt to clawback more tokens than user1 holds should fail.
    client.clawback(&admin, &user1, &100_000);
}
