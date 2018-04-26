echo 'Creating listing contract' && \
./request.sh payloads/origin/create_listing_contract.json && \

echo 'Get listing count' && \
./request.sh payloads/origin/listings_length.json && \

echo 'Creating new listing (raw)' && \
./request.sh payloads/origin/create_new_listing_raw_transaction.json && \

echo 'Get listing ount' && \
./request.sh payloads/origin/listings_length.json
