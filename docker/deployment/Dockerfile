FROM docker.io/oasislabs/testnet:latest

ADD . /ekiden
# hack for web3-client hardcoded paths
RUN ln -s /ekiden/res /resources
