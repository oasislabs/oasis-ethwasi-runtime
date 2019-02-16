ARG OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG=staging-hw

FROM oasislabs/ekiden-runtime-ethereum:${OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG}

COPY docker/all-in-one/service /etc/service
COPY \
    resources/keymanager/km-key.pem \
    resources/keymanager/km.pem \
    scripts/gateway.sh \
    scripts/utils.sh \
    /etc/service/gateway/

EXPOSE 8545/tcp 8555/tcp
