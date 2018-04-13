echo 'Creating counter contract' && \
./request.sh payloads/counter/create_counter_contract.json && \

echo 'Get count' && \
./request.sh payloads/counter/get_count.json && \

echo 'Increment count' && \
./request.sh payloads/counter/increment_count.json && \

echo 'Get count' && \
./request.sh payloads/counter/get_count.json
