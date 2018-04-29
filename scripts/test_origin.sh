echo 'Creating listing contract' && \
./request.sh payloads/origin/1_*.json && \

sleep 1s
echo ''
echo 'Creating counter contract (to increase nonce)' && \
./request.sh payloads/counter/create_counter_contract.json && \

sleep 2s
echo ''
echo 'Get owner' && \
./request.sh payloads/origin/2_*.json

echo ''
echo 'Get listings length' && \
./request.sh payloads/origin/3_*.json

echo ''
echo 'Estimating gas for creating new listing' && \
./request.sh payloads/origin/4_*.json

### Note: this raw transaction uses nonce = 2. If you reorder the transactions in this
### script, you must ensure the 0x711...2b account has current nonce value 2 or Sputnik
### will reject this transaction.
echo ''
echo 'Creating new listing' && \
./request.sh payloads/origin/5_*.json

echo ''
echo 'Get listings length' && \
./request.sh payloads/origin/3_*.json

