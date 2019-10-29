#!/bin/bash

if [[ -z "$1" ]]; then
  echo "Usage: $0 [method] [params]"
  exit 1
fi

METHOD="$1"
IFS=','
shift

PARAMS="$*"

PAYLOAD=`cat <<EOF
{
  "id": 12345,
  "jsonrpc": "2.0",
  "params": [
    $PARAMS
  ],
  "method": "$METHOD"
}
EOF
`

echo "$PAYLOAD" | curl -s -X POST http://localhost:8545 -d @- --header "Content-Type: application/json"
