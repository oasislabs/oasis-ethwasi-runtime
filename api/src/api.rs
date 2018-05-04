use ekiden_core::contract::contract_api;

contract_api! {
    pub fn init_genesis_state(InitStateRequest) -> InitStateResponse;

    pub fn execute_transaction(ExecuteTransactionRequest) -> ExecuteTransactionResponse;
}
