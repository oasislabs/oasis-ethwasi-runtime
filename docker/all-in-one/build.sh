#!/bin/sh

tar -c \
    configs \
    docker/all-in-one/service \
    docker/all-in-one/Dockerfile \
| docker build \
    --pull \
    --file=docker/all-in-one/Dockerfile \
    --tag=local-aio:latest \
    -
