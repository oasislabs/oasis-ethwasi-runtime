# All-in-one image

This Docker image runs an entire testnet and exposes a web3 interface.

## Instructions

### Install Intel SGX driver on host
It's a kernel driver.

After it's installed, `/dev/isgx` should be present.

The image runs aesmd internally, so you don't need to install libenclave-common (sometimes called the PSW) on the host.
The image has compiled binaries, so you don't need to install the Intel SGX SDK.

### Set up IAS credentials
Get an SPID and TLS certificate and key for communicating with Intel Attestation Service (IAS) for **linkable** signatures.

Enter these in a directory (we'll suppose the path `/opt/oasis/ias-creds` in these instructions) as three files.

1. Your SPID, in hex, in `spid.txt`
2. Your TLS certificate, in PEM, in `tls-cert.pem`
3. Your TLS private key, in PEM, in `tls-key.pem`

You'll mount this directory into the container.

### Enable user namespaces on host
Run

```
sysctl kernel.unprivileged_userns_clone=1
```

The software uses namespaces to isolate some parts of itself.

### Create the container
Run

```
docker run \
    --detach \
    --rm \
    --name oasis-local \
    --security-opt=apparmor=unconfined \
    --security-opt=seccomp=unconfined \
    --volume=/opt/oasis/ias-creds:/mnt/ias-creds \
    --device=/dev/isgx \
    --publish=127.0.0.1:8545:8545/tcp \
    --publish=127.0.0.1:8555:8555/tcp \
    oasislabs/gateway-all-in-one:staging-hw
```

The unconfined AppArmor and seccomp options allow the software create a process with reduced privileges.

### Access web3
You can now access the network through a local web3 endpoint.
For example,

```
curl -s \
    -X POST \
    http://127.0.0.1:8545 \
    -d @- \
    --header "Content-Type: application/json" \
    <<EOF
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_getBalance",
  "params": [
    "0x1cca28600d7491365520b31b466f88647b9839ec",
    "latest"
  ]
}
EOF
```

Should give a result like
```
{"jsonrpc":"2.0","result":"0x56bc75e2d63100000","id":1}
```

### Spend DEV
The genesis block contains the following accounts:

* `7110316b618d20d0c44728ac2a3d683536ea682b` is a test account with the following
  private key: `533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c`.

* `1cca28600d7491365520b31b466f88647b9839ec` is a test account with the following
  private key: `c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179`.

### Stop the container
Run

```
docker stop oasis-local
```
