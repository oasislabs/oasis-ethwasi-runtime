ARG OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG=staging

FROM oasislabs/ekiden-runtime-ethereum:${OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG}

COPY docker/all-in-one/service /etc/service
COPY configs/single_node /var/ekiden/all-in-one-sw
COPY configs/single_node_sgx /var/ekiden/all-in-one-hw
RUN chmod -R go-rwx /var/ekiden/all-in-one-sw /var/ekiden/all-in-one-hw

EXPOSE 8545/tcp 8555/tcp
