#!/bin/bash

if [[ -z "$1" ]]; then
  echo "Usage: $0 [payload file]"
  exit 1
fi

curl -s -X POST http://localhost:8545 -d @$1 --header "Content-Type: application/json"
