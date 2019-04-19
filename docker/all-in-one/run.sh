#!/bin/sh

docker run \
    --detach \
    --rm \
    --name test \
    --security-opt=apparmor=unconfined \
    --security-opt=seccomp=unconfined \
    --volume="$PWD/../private-ops/untracked/ias-dev-creds:/mnt/ias-creds" \
    --device=/dev/isgx \
    --publish=127.0.0.1:8545:8545/tcp \
    --publish=127.0.0.1:8546:8546/tcp \
    local-aio:latest
