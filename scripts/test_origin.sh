#!/bin/bash

# Address of test account. Chain must include this account with positive Ether balance.
TEST_ACCOUNT="0x7110316b618d20d0c44728ac2a3d683536ea682b"

exit_error() {
  echo $1 >&2
  exit 1
}

# Make sure we have jq installed.
command -v jq >/dev/null 2>&1 || exit_error "This script requires jq to parse JSON output. Please install with 'brew install jq' (or equivalent for your platform). Aborting."

echo "Checking test account balance $TEST_ACCOUNT"
RESULT=`./request_raw.sh eth_getBalance \"$TEST_ACCOUNT\" \"latest\" | jq -e .result` || exit_error "Test account doesn't exist or eth_getBalance error"
echo "> Balance: $RESULT"

echo -e '\nCreating ListingsRegistry contract'
HASH=`./request.sh payloads/origin/create_listings_registry.json | jq -e .result` || exit_error "Error creating listing contract"
echo "> Transaction hash: $HASH"

echo -e "\ngetTransactionReceipt: $HASH"
ADDR=`./request_raw.sh eth_getTransactionReceipt $HASH | jq -e .result.contractAddress` || exit_error "Error getting transaction receipt"
echo "> Contract address: $ADDR"

echo -e "\ngetTransactionByHash: $HASH"
RESULT=`./request_raw.sh eth_getTransactionByHash $HASH | jq -e .result.hash` || exit_error "Error getting transaction receipt"
[ $RESULT == $HASH ] || exit_error "Error: hash mismatch"
echo "> OK"

echo -e '\nCreating counter contract (to increase nonce)'
./request.sh payloads/counter/create_counter_contract.json | jq -e .result >/dev/null || exit_error "Error creating counter contract"
echo "> OK"

echo -e "\nGet listings length ($ADDR)"
RESULT=`./request.sh payloads/origin/call_listings_length.json | jq -e .result`
echo "> Result: $RESULT"
[ $RESULT == '"0x0000000000000000000000000000000000000000000000000000000000000000"' ] || exit_error "Error: listings length should be 0"

echo -e '\nEstimate gas for creating new listing'
RESULT=`./request.sh payloads/origin/estimate_gas_create_listing.json | jq -e .result` || exit_error "Error estimating gas"
echo "> Result: $RESULT"

echo -e "\nGet transaction count for address $TEST_ACCOUNT"
RESULT=`./request_raw.sh eth_getTransactionCount \"$TEST_ACCOUNT\" \"latest\" | jq -e .result` || exit_error "Error getting transaction count"
echo "> Result: $RESULT"
[ $RESULT == '"0x2"' ] || exit_error "Error: incorrect transactions count"

# Note: this raw transaction is signed with nonce = 2. If you reorder the transactions in this script,
# you must ensure the test account has current nonce value 2 at this point (see getTransactionCount call above).
echo -e "\nCreating new listing ($ADDR)"
HASH=`./request.sh payloads/origin/create_listing.json | jq -e .result` || exit_error "Error creating new listing"
echo "> Transaction hash: $HASH"

echo -e "\nGet listings length ($ADDR)"
RESULT=`./request.sh payloads/origin/call_listings_length.json | jq -e .result`
echo "> Result: $RESULT"
[ $RESULT == '"0x0000000000000000000000000000000000000000000000000000000000000001"' ] || exit_error "Error: listings length should be 1"

echo -e "\nSuccess!"
