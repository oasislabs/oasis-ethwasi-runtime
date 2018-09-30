use client::Client;
use ekiden_common::bytes::B512;
use ekiden_core::futures::FutureExt;
use ekiden_keymanager_common::confidential;
use ethereum_types::Address;
use impls::eth::EthClient;
use jsonrpc_core::futures::{future, Future};
use jsonrpc_core::{BoxFuture, Error, ErrorCode, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::helpers::errors;
use parity_rpc::v1::metadata::Metadata;
use parity_rpc::v1::traits::Eth;
use parity_rpc::v1::types::{BlockNumber, Bytes, CallRequest, H256};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use traits::confidential::{Confidential, PublicKeyResult};

pub struct ConfidentialClient {
    client: Arc<Client>,
    eth_client: EthClient,
}

impl ConfidentialClient {
    pub fn new(client: Arc<Client>) -> Self {
        ConfidentialClient {
            client: client.clone(),
            eth_client: EthClient::new(&client),
        }
    }
}
impl Confidential for ConfidentialClient {
    type Metadata = Metadata;

    fn public_key(&self, contract: Address) -> Result<PublicKeyResult> {
        measure_counter_inc!("confidential_getPublicKey");
        let (public_key, _) = confidential::default_contract_keys();
        // TODO: V1 should be issued by the key manager
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(PublicKeyResult {
            public_key,
            timestamp,
            signature: B512::from(0), // TODO: V1
        })
    }

    fn call_enc(
        &self,
        meta: Self::Metadata,
        req: CallRequest,
        tag: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        measure_counter_inc!("confidential_call");
        measure_histogram_timer!("confidential_call_enc_time");
        let encrypted_calldata = req.data.map(|d: Bytes| d.0);
        match confidential::decrypt(encrypted_calldata) {
            Err(_) => future::err(Error::new(ErrorCode::InternalError)).boxed(),
            Ok(decryption) => {
                let unencrypted_req = CallRequest {
                    from: req.from,
                    to: req.to,
                    gas_price: req.gas_price,
                    gas: req.gas,
                    value: req.value,
                    data: Some(Bytes::from(decryption.plaintext.clone())),
                    nonce: req.nonce,
                };
                self.eth_client
                    .call(meta, unencrypted_req, tag)
                    .and_then(|data: Bytes| {
                        future::done(
                            confidential::encrypt(
                                data.0,
                                decryption.nonce,
                                decryption.peer_public_key,
                            ).map(Bytes::from)
                                .map_err(|_| Error::new(ErrorCode::InternalError)),
                        )
                    })
                    .boxed()
            }
        }
    }
    fn send_raw_transaction(&self, raw: Bytes) -> Result<H256> {
        measure_counter_inc!("confidential_sendRawTransaction");
        measure_histogram_timer!("confidential_sendRawTransaction_time");
        if log_enabled!(log::Level::Debug) {
            debug!("confidential_sendRawTransaction(data: {:?})", raw);
        } else {
            info!("confidential_sendRawTransaction(data: ...)");
        }
        self.client
            .send_raw_transaction(raw.into(), true)
            .map(Into::into)
            .map_err(errors::execution)
    }
}
