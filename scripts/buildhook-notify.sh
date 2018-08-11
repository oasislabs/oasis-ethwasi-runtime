#!/bin/bash
##
# Triggers the ops-production build hook service
#
# Usage:
#   ./buildhook-notify.sh <target> <repository> <tag> <secret_token>
##

target=$1
repository=$2
tag=$3
secret_token=$4

curl -d '{"target":"'"$target"'","repository":"'"$repository"'","tag":"'"$tag"'"}' -H "Content-Type: application/json" -X POST https://buildhook.ops-production.oasiscloud.io/webhook?token=$secret_token