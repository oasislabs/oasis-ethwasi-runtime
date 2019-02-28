#!/bin/sh

tar -c \
    resources/keymanager/km-key.pem \
    resources/keymanager/km.pem \
    scripts/gateway.sh \
    scripts/utils.sh \
    docker/all-in-one/service \
    docker/all-in-one/Dockerfile \
| docker build \
    --file=docker/all-in-one/Dockerfile \
    --tag=oasislabs/gateway-all-in-one:testing-hw \
    -
