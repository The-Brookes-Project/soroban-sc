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
    env.mock_all_auths(); // Add auth mocking
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
    env.mock_all_auths(); // Move auth mocking to the beginning
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
    env.mock_all_auths(); // Move auth mocking to the beginning
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

    // Set KYC and compliance status
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
    client.purchase(&buyer, &buyer, &purchase_amount);

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

#[test]
fn test_purchase_with_different_beneficiary() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;
    let usdc_token_admin_client = usdc_token.1;

    let current_ledger = env.ledger().sequence();
    let expiration_ledger = current_ledger + 100;

    usdc_token_client.approve(
        &buyer,
        &contract_id,
        &1_000_000_000i128,
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

    // Set KYC and compliance status for all parties
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_kyc_status(&admin, &beneficiary, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &beneficiary, &ComplianceStatus::Approved);

    // Initial balances
    let initial_buyer_token_balance = client.balance(&buyer);
    let initial_beneficiary_token_balance = client.balance(&beneficiary);
    let initial_issuer_token_balance = client.balance(&issuer);
    let initial_buyer_usdc_balance = usdc_token_client.balance(&buyer);
    assert_eq!(initial_buyer_token_balance, 0);
    assert_eq!(initial_beneficiary_token_balance, 0);
    assert_eq!(initial_issuer_token_balance, 1_000_000_000_000);
    assert_eq!(initial_buyer_usdc_balance, 1_000_000_000);

    // Buyer purchases 500,000 tokens for beneficiary (50,000,000 USDC at 0.1 USDC per token)
    let purchase_amount = 500_000_000;
    client.purchase(&buyer, &beneficiary, &purchase_amount);

    // Check token balances after purchase
    // Buyer should have 0 tokens (didn't receive any)
    // Beneficiary should have the purchased tokens
    let buyer_token_balance = client.balance(&buyer);
    let beneficiary_token_balance = client.balance(&beneficiary);
    let issuer_token_balance = client.balance(&issuer);
    let buyer_usdc_balance = usdc_token_client.balance(&buyer);
    let contract_usdc_balance = client.usdc_balance();

    assert_eq!(buyer_token_balance, 0); // Buyer didn't receive tokens
    assert_eq!(beneficiary_token_balance, 500_000_000); // Beneficiary received tokens
    assert_eq!(issuer_token_balance, 999_500_000_000);
    assert_eq!(buyer_usdc_balance, 950_000_000); // Buyer paid USDC: 1B - 50M
    assert_eq!(contract_usdc_balance, 50_000_000);
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
    client.purchase(&buyer, &buyer, &purchase_amount);
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

// ===== Additional Test Coverage =====

#[test]
fn test_add_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Add new admin
    client.add_admin(&admin, &new_admin);

    // Verify new admin can perform admin functions
    client.set_kyc_status(&new_admin, &issuer, &true);
    client.set_compliance_status(&new_admin, &issuer, &ComplianceStatus::Approved);
}

#[test]
#[should_panic]
fn test_add_admin_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Non-admin tries to add admin should fail
    client.add_admin(&non_admin, &new_admin);
}

#[test]
#[should_panic]
fn test_add_admin_duplicate() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Try to add admin again should fail
    client.add_admin(&admin, &admin);
}

#[test]
fn test_configure_authorization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up users with KYC and compliance
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_kyc_status(&admin, &user2, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user2, &ComplianceStatus::Approved);

    // Disable transfer restrictions first
    client.set_transfer_restriction(&admin, &false);

    // Configure authorization to not require it
    client.configure_authorization(&admin, &false, &false);

    // Transfer should work without KYC/compliance checks
    client.transfer(&issuer, &user1, &100_000);
    client.transfer(&user1, &user2, &50_000);

    // Re-enable authorization
    client.configure_authorization(&admin, &true, &true);

    // Transfer should still work since users are already verified
    client.transfer(&user2, &user1, &25_000);
}

#[test]
#[should_panic]
fn test_configure_authorization_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Non-admin tries to configure authorization should fail
    client.configure_authorization(&non_admin, &false, &false);
}

#[test]
fn test_view_functions() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Test check_compliance for different statuses
    let issuer_compliance = client.check_compliance(&issuer);
    assert_eq!(issuer_compliance, ComplianceStatus::Pending);

    // Set compliance status and test
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    let user1_compliance = client.check_compliance(&user1);
    assert_eq!(user1_compliance, ComplianceStatus::Approved);

    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Rejected);
    let user1_compliance_rejected = client.check_compliance(&user1);
    assert_eq!(user1_compliance_rejected, ComplianceStatus::Rejected);

    // Test is_kyc_verified
    let issuer_kyc = client.is_kyc_verified(&issuer);
    assert_eq!(issuer_kyc, false);

    client.set_kyc_status(&admin, &user1, &true);
    let user1_kyc = client.is_kyc_verified(&user1);
    assert_eq!(user1_kyc, true);

    client.set_kyc_status(&admin, &user1, &false);
    let user1_kyc_false = client.is_kyc_verified(&user1);
    assert_eq!(user1_kyc_false, false);
}

#[test]
#[should_panic]
fn test_initialize_validation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;
    let usdc_address = usdc_token_client.address.clone(); // Clone the address

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
            usdc_address.clone() // Use cloned address
        )
    });

    // Test that double initialization fails
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token 2"),
            String::from_str(&env, "SCTY2"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address // Use original address
        )
    });
}

#[test]
#[should_panic]
fn test_initialize_invalid_parameters() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Initialize token with invalid parameters
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            8, // Invalid: decimals > 7
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Home domain cannot be empty")]
fn test_initialize_empty_home_domain() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Initialize token with empty home_domain
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, ""), // Empty home_domain should panic
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Name cannot be empty")]
fn test_initialize_empty_name() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Initialize token with empty name
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, ""), // Empty name should panic
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Symbol cannot be empty")]
fn test_initialize_empty_symbol() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Initialize token with empty symbol
    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, ""), // Empty symbol should panic
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Name cannot exceed 64 characters")]
fn test_initialize_name_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Create a name longer than 64 characters
    let long_name = String::from_str(&env, "This is a very long token name that exceeds the maximum allowed length of 64 characters");

    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            long_name,
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Symbol cannot exceed 12 characters")]
fn test_initialize_symbol_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "VERYLONGSYMBOL"), // 14 characters
            6,
            1_000_000_000_000,
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Home domain cannot exceed 256 characters")]
fn test_initialize_home_domain_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    // Create a home_domain longer than 256 characters (this string is 260+ chars)
    let long_domain = String::from_str(&env, "this-is-a-very-long-domain-name-that-exceeds-the-maximum-allowed-length-of-256-characters-and-should-trigger-the-validation-error-when-initializing-the-token-contract-because-we-need-to-ensure-proper-bounds-checking-for-all-string-parameters-in-the-code.example.com");

    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000,
            issuer.clone(),
            long_domain,
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Total supply cannot exceed 1 quintillion")]
fn test_initialize_total_supply_too_large() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    env.as_contract(&contract_id, || {
        SecurityTokenContract::initialize(
            env.clone(),
            String::from_str(&env, "Security Token"),
            String::from_str(&env, "SCTY"),
            6,
            1_000_000_000_000_000_001, // Exceeds 1 quintillion
            issuer.clone(),
            String::from_str(&env, "example.com"),
            admin.clone(),
            100_000,
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "USDC price cannot exceed 1 trillion")]
fn test_initialize_usdc_price_too_large() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

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
            1_000_000_000_001, // Exceeds 1 trillion
            usdc_token_client.address
        )
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #24)")]
fn test_transfer_edge_cases() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up user
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Disable transfer restrictions
    client.set_transfer_restriction(&admin, &false);

    // Transfer to user1
    client.transfer(&issuer, &user1, &100_000);

    // Check initial balance
    let initial_balance = client.balance(&user1);
    assert_eq!(initial_balance, 100_000);

    // Self-transfers are now blocked (Error #24)
    // This test now expects a panic with error code 24
    client.transfer(&user1, &user1, &10_000);
}

#[test]
#[should_panic]
fn test_transfer_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up users
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_kyc_status(&admin, &user2, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user2, &ComplianceStatus::Approved);

    // Transfer small amount to user1
    client.transfer(&issuer, &user1, &50_000);

    // Try to transfer more than user1 has
    client.transfer(&user1, &user2, &100_000);
}

#[test]
#[should_panic]
fn test_transfer_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up user
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Try to transfer zero amount
    client.transfer(&issuer, &user1, &0);
}

#[test]
fn test_withdraw_usdc_edge_cases() {
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
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

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

    // Set up buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Make a purchase to accumulate USDC
    client.purchase(&buyer, &buyer, &500_000_000);

    // Test partial withdrawal
    client.withdraw_usdc(&admin, &25_000_000);
    assert_eq!(client.usdc_balance(), 25_000_000);

    // Test full withdrawal
    client.withdraw_usdc(&admin, &25_000_000);
    assert_eq!(client.usdc_balance(), 0);
}

#[test]
#[should_panic]
fn test_withdraw_usdc_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

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
            100_000,
            usdc_token_client.address
        )
    });

    // Non-admin tries to withdraw USDC
    client.withdraw_usdc(&non_admin, &10_000_000);
}

#[test]
#[should_panic]
fn test_withdraw_usdc_exceeds_balance() {
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
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

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

    // Set up buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Make a purchase to accumulate USDC
    client.purchase(&buyer, &buyer, &500_000_000);

    // Try to withdraw more than available
    client.withdraw_usdc(&admin, &100_000_000);
}

// ===== Additional Tests for 100% Coverage =====

#[test]
#[should_panic]
fn test_purchase_negative_amount() {
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
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

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

    // Set up buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Try to purchase negative amount
    client.purchase(&buyer, &buyer, &-100_000);
}

#[test]
#[should_panic]
fn test_purchase_zero_amount() {
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
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

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

    // Set up buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Try to purchase zero amount
    client.purchase(&buyer, &buyer, &0);
}



#[test]
#[should_panic]
fn test_withdraw_usdc_negative_amount() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Try to withdraw negative amount
    client.withdraw_usdc(&admin, &-10_000_000);
}

#[test]
#[should_panic]
fn test_withdraw_usdc_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Try to withdraw zero amount
    client.withdraw_usdc(&admin, &0);
}

#[test]
#[should_panic]
fn test_transfer_negative_amount() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up user
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Try to transfer negative amount
    client.transfer(&issuer, &user1, &-100_000);
}

#[test]
fn test_compliance_status_transitions() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Test all compliance status transitions
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Pending);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Pending);

    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Approved);

    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Rejected);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Rejected);

    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Suspended);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Suspended);
}

#[test]
fn test_kyc_status_transitions() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Test KYC status transitions
    assert_eq!(client.is_kyc_verified(&user1), false);

    client.set_kyc_status(&admin, &user1, &true);
    assert_eq!(client.is_kyc_verified(&user1), true);

    client.set_kyc_status(&admin, &user1, &false);
    assert_eq!(client.is_kyc_verified(&user1), false);
}

#[test]
fn test_transfer_restriction_toggle() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Set up user
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);

    // Test transfer restriction toggle
    // Initially restricted (default)
    client.transfer(&issuer, &user1, &100_000); // Admin can transfer

    // Disable restrictions
    client.set_transfer_restriction(&admin, &false);
    client.transfer(&user1, &issuer, &50_000); // User can transfer

    // Re-enable restrictions
    client.set_transfer_restriction(&admin, &true);
    client.transfer(&issuer, &user1, &50_000); // Admin can transfer
}

#[test]
fn test_metadata_retrieval() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;
    let usdc_address = usdc_token_client.address.clone(); // Clone the address

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
            usdc_address.clone() // Use cloned address
        )
    });

    // Test metadata retrieval
    let metadata = client.get_metadata();
    assert_eq!(metadata.name, String::from_str(&env, "Security Token"));
    assert_eq!(metadata.symbol, String::from_str(&env, "SCTY"));
    assert_eq!(metadata.decimals, 6);
    assert_eq!(metadata.total_supply, 1_000_000_000_000);
    assert_eq!(metadata.issuer, issuer);
    assert_eq!(metadata.home_domain, String::from_str(&env, "example.com"));
    assert_eq!(metadata.usdc_price, 100_000);
    assert_eq!(metadata.usdc_token, usdc_token_client.address); // Use original address
}

#[test]
fn test_balance_queries() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Test balance queries
    assert_eq!(client.balance(&issuer), 1_000_000_000_000);
    assert_eq!(client.balance(&user1), 0);

    // Transfer and check balance
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &user1, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    client.set_transfer_restriction(&admin, &false);

    client.transfer(&issuer, &user1, &100_000);
    assert_eq!(client.balance(&user1), 100_000);
    assert_eq!(client.balance(&issuer), 999_999_900_000); // Fixed calculation
}

#[test]
fn test_usdc_balance_tracking() {
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
        &contract_id,
        &1_000_000_000i128,
        &expiration_ledger
    );

    // Mint USDC to buyer
    usdc_token_admin_client.mint(&buyer, &1_000_000_000);

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

    // Set up buyer
    client.set_kyc_status(&admin, &issuer, &true);
    client.set_kyc_status(&admin, &buyer, &true);
    client.set_compliance_status(&admin, &issuer, &ComplianceStatus::Approved);
    client.set_compliance_status(&admin, &buyer, &ComplianceStatus::Approved);

    // Test USDC balance tracking
    assert_eq!(client.usdc_balance(), 0);

    // Make purchases and track balance
    client.purchase(&buyer, &buyer, &500_000_000);
    assert_eq!(client.usdc_balance(), 50_000_000);

    client.purchase(&buyer, &buyer, &300_000_000);
    assert_eq!(client.usdc_balance(), 80_000_000);

    // Withdraw and check balance
    client.withdraw_usdc(&admin, &30_000_000);
    assert_eq!(client.usdc_balance(), 50_000_000);
}

#[test]
fn test_token_price_retrieval() {
    let env = Env::default();
    env.mock_all_auths();
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
            100_000,
            usdc_token_client.address
        )
    });

    // Test token price retrieval
    assert_eq!(client.token_price(), 100_000);
}

#[test]
#[should_panic]
fn test_authorization_revocable_kyc_revoke_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with authorization_revocable = true (default)
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
            usdc_token_client.address
        )
    });

    // Grant KYC
    client.set_kyc_status(&admin, &user1, &true);
    assert_eq!(client.is_kyc_verified(&user1), true);

    // Configure authorization to be non-revocable
    client.configure_authorization(&admin, &true, &false);

    // Attempt to revoke KYC should fail
    client.set_kyc_status(&admin, &user1, &false);
}

#[test]
#[should_panic]
fn test_authorization_revocable_compliance_revoke_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with authorization_revocable = true (default)
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
            usdc_token_client.address
        )
    });

    // Grant compliance
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Approved);

    // Configure authorization to be non-revocable
    client.configure_authorization(&admin, &true, &false);

    // Attempt to revoke compliance should fail
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Rejected);
}

#[test]
fn test_authorization_revocable_kyc_revoke_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with authorization_revocable = true (default)
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
            usdc_token_client.address
        )
    });

    // Grant KYC
    client.set_kyc_status(&admin, &user1, &true);
    assert_eq!(client.is_kyc_verified(&user1), true);

    // Revoke KYC should succeed (authorization_revocable = true by default)
    client.set_kyc_status(&admin, &user1, &false);
    assert_eq!(client.is_kyc_verified(&user1), false);
}

#[test]
fn test_authorization_revocable_compliance_revoke_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SecurityTokenContract, ());
    let issuer = Address::generate(&env);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    // Setup test USDC token contract
    let usdc_token = create_token_contract(&env, &admin);
    let usdc_token_client = usdc_token.0;

    let client = SecurityTokenContractClient::new(&env, &contract_id);

    // Initialize token with authorization_revocable = true (default)
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
            usdc_token_client.address
        )
    });

    // Grant compliance
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Approved);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Approved);

    // Revoke compliance should succeed (authorization_revocable = true by default)
    client.set_compliance_status(&admin, &user1, &ComplianceStatus::Rejected);
    assert_eq!(client.check_compliance(&user1), ComplianceStatus::Rejected);
}
