#!/bin/sh

docker run \
    --detach \
    --rm \
    --name test \
    --security-opt=apparmor=unconfined \
    --security-opt=seccomp=unconfined \
    --publish=127.0.0.1:8545:8545/tcp \
    --publish=127.0.0.1:8555:8555/tcp \
    -e AIO_NOSGX=1 \
    local-aio:latest
