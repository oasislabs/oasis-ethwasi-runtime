use ekiden_core::contract::contract_api;

contract_api! {
    pub fn genesis_block_initialized(bool) -> bool;
    pub fn init_genesis_block(InitStateRequest) -> InitStateResponse;

    pub fn execute_transaction(ExecuteTransactionRequest) -> ExecuteTransactionResponse;

    pub fn execute_raw_transaction(ExecuteRawTransactionRequest) -> ExecuteTransactionResponse;

    pub fn get_transaction_receipt(ReceiptRequest) -> ReceiptResponse;

    pub fn get_account_balance(AccountRequest) -> AccountBalanceResponse;

    pub fn get_account_nonce(AccountRequest) -> AccountNonceResponse;
}
