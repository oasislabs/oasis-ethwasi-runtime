use ekiden_core::rpc::rpc_api;

rpc_api! {
    metadata {
        name = helloworld;
        version = "0.1.0";
        client_attestation_required = false;
    }

    rpc hello_world(HelloWorldRequest) -> HelloWorldResponse;
}
