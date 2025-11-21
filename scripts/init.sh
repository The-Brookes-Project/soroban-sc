#!/bin/bash

# Default to testnet
NETWORK=${1:-testnet}
SOURCE=${2:-ADMIN}
USDC_ISSUER="GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
USDC_CODE="USDC"

# Colors
GREEN='\033[0;32m'
NC='\033[0m'

echo -e "${GREEN}Starting initialization for $NETWORK...${NC}"

# 1. Load Contract IDs
if [ -f ".contracts_$NETWORK" ]; then
    source ".contracts_$NETWORK"
else
    echo "Error: .contracts_$NETWORK file not found. Run deploy.sh first."
    exit 1
fi

# 2. Get USDC Contract ID
echo "Deriving USDC Contract ID..."
# Note: This requires the network to be configured in stellar-cli or passed explicitly
# We use the --network flag which relies on ~/.config/stellar/network-config.toml or default networks
USDC_ID=$(stellar contract id asset --asset "$USDC_CODE:$USDC_ISSUER" --network $NETWORK)

if [ -z "$USDC_ID" ]; then
    echo "Failed to get USDC Contract ID"
    exit 1
fi
echo "USDC Contract ID: $USDC_ID"

# 3. Initialize KYC
echo -e "${GREEN}Initializing KYC Contract ($KYC_ID)...${NC}"
stellar contract invoke \
    --id $KYC_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- initialize \
    --admin $SOURCE

# 3a. Set KYC Status for ADMIN
echo -e "${GREEN}Setting KYC verified status for ADMIN...${NC}"
stellar contract invoke \
    --id $KYC_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- set_kyc_status \
    --admin $SOURCE \
    --user $SOURCE \
    --verified true

# 3b. Set Compliance Status for ADMIN
echo -e "${GREEN}Setting compliance status to Approved for ADMIN...${NC}"
stellar contract invoke \
    --id $KYC_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- set_compliance_status \
    --admin $SOURCE \
    --user $SOURCE \
    --status Approved

# 4. Initialize Vault
echo -e "${GREEN}Initializing Vault Contract ($VAULT_ID)...${NC}"
stellar contract invoke \
    --id $VAULT_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- initialize \
    --admin $SOURCE \
    --stablecoin_address $USDC_ID

# 5. Initialize Property
# Using minimum price (e.g. 1 unit = 0.0000001 USDC if 7 decimals)
# Total supply: 1,000,000 tokens
# Decimals: 7 (matching USDC usually)
echo -e "${GREEN}Initializing Property Contract ($PROPERTY_ID)...${NC}"
stellar contract invoke \
    --id $PROPERTY_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- initialize \
    --admin $SOURCE \
    --name "Verseprop Test Token" \
    --symbol "VPT" \
    --decimals 7 \
    --total_supply 10000000000000 \
    --token_price 10000000 \
    --vault_address $VAULT_ID \
    --kyc_address $KYC_ID \
    --stablecoin_address $USDC_ID

# 6. Authorize Property in Vault
echo -e "${GREEN}Authorizing Property in Vault...${NC}"
stellar contract invoke \
    --id $VAULT_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- authorize_property \
    --admin $SOURCE \
    --property_contract $PROPERTY_ID

echo -e "${GREEN}Initialization complete!${NC}"
