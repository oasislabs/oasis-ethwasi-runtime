To build the `all-in-one:testing` image, run:
```sh
./docker/all-in-one/build.sh
```

We have some scripts containing example `docker run` invocations for the image:

* **run.sh** runs the image, where IAS credentials are mounted from `../private-ops/untracked/ias-dev-creds` (an arbitrary choice).
* **run-sw.sh** runs the image in an upcoming non-TEE configuration.
