#!/bin/bash

# Default to testnet if no network is specified
NETWORK=${1:-testnet}
SOURCE=${2:-ADMIN}

# Colors for output
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting deployment process for $NETWORK...${NC}"

# 1. Build Contracts
echo -e "${GREEN}Building contracts...${NC}"
stellar contract build
if [ $? -ne 0 ]; then
    echo "Build failed. Exiting."
    exit 1
fi

# 2. Deploy Contracts
echo -e "${GREEN}Deploying contracts to $NETWORK using identity $SOURCE...${NC}"

# Deploy KYC
echo "Deploying verse_kyc..."
KYC_ID=$(stellar contract deploy --wasm target/wasm32v1-none/release/verse_kyc.wasm --source $SOURCE --network $NETWORK)
if [ -z "$KYC_ID" ]; then
    echo "Failed to deploy verse_kyc"
    exit 1
fi
echo "KYC Contract ID: $KYC_ID"

# Deploy Vault
echo "Deploying verse_vault..."
VAULT_ID=$(stellar contract deploy --wasm target/wasm32v1-none/release/verse_vault.wasm --source $SOURCE --network $NETWORK)
if [ -z "$VAULT_ID" ]; then
    echo "Failed to deploy verse_vault"
    exit 1
fi
echo "Vault Contract ID: $VAULT_ID"

# Deploy Property
echo "Deploying verse_property..."
PROP_ID=$(stellar contract deploy --wasm target/wasm32v1-none/release/verse_property.wasm --source $SOURCE --network $NETWORK)
if [ -z "$PROP_ID" ]; then
    echo "Failed to deploy verse_property"
    exit 1
fi
echo "Property Contract ID: $PROP_ID"

# 3. Save Output
OUTPUT_FILE=".contracts_$NETWORK"
echo "KYC_ID=$KYC_ID" > $OUTPUT_FILE
echo "VAULT_ID=$VAULT_ID" >> $OUTPUT_FILE
echo "PROPERTY_ID=$PROP_ID" >> $OUTPUT_FILE

echo -e "${GREEN}Deployment complete!${NC}"
echo "Contract IDs have been saved to $OUTPUT_FILE"
echo "You can now initialize the property contract using these IDs."
