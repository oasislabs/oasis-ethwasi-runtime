To build and push the `ekiden-runtime-ethereum:testing-hw` image, run:
```sh
SGX_MODE=HW \
./docker/ekiden-runtime-ethereum/docker_build_and_push.sh \
master \
testing-hw \
../docker-build-key latest-hw
```

Then, to build the `all-in-one:testing-hw` image, run:
```sh
./docker/all-in-one/build.sh
```
