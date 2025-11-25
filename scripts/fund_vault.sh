#!/bin/bash

# Default to testnet
NETWORK=${1:-testnet}
SOURCE=${2:-ADMIN}
USDC_ISSUER="GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
USDC_CODE="USDC"

# Amount to fund: $10 USDC with 7 decimals = 100000000
AMOUNT=100000000

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}Funding vault with $10 USDC on $NETWORK...${NC}"

# 1. Load Contract IDs
if [ -f ".contracts_$NETWORK" ]; then
    source ".contracts_$NETWORK"
else
    echo "Error: .contracts_$NETWORK file not found. Run deploy.sh first."
    exit 1
fi

echo -e "${YELLOW}Vault Contract ID: $VAULT_ID${NC}"

# 2. Get USDC Contract ID
echo "Deriving USDC Contract ID..."
USDC_ID=$(stellar contract id asset --asset "$USDC_CODE:$USDC_ISSUER" --network $NETWORK)

if [ -z "$USDC_ID" ]; then
    echo "Failed to get USDC Contract ID"
    exit 1
fi
echo -e "${YELLOW}USDC Contract ID: $USDC_ID${NC}"

# 3. Fund Vault via fund_vault method
echo -e "${GREEN}Funding vault with $10 USDC ($AMOUNT stroops) via fund_vault method...${NC}"
stellar contract invoke \
    --id $VAULT_ID \
    --source $SOURCE \
    --network $NETWORK \
    -- fund_vault \
    --admin $SOURCE \
    --amount $AMOUNT

if [ $? -eq 0 ]; then
    echo -e "${GREEN}Successfully funded vault with $10 USDC!${NC}"
else
    echo "Failed to fund vault"
    exit 1
fi

