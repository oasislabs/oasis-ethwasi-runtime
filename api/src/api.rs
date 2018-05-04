use ekiden_core::contract::contract_api;

contract_api! {
    pub fn genesis_block_initialized(bool) -> bool;
    pub fn init_genesis_block(InitStateRequest) -> InitStateResponse;

    pub fn execute_transaction(ExecuteTransactionRequest) -> ExecuteTransactionResponse;

    pub fn execute_raw_transaction(ExecuteRawTransactionRequest) -> ExecuteTransactionResponse;
}
