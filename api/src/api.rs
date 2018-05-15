use ekiden_core::contract::contract_api;

contract_api! {
    pub fn genesis_block_initialized(bool) -> bool;
    pub fn init_genesis_block(InitStateRequest) -> InitStateResponse;

    pub fn debug_execute_unsigned_transaction(ExecuteTransactionRequest) -> ExecuteTransactionResponse;

    pub fn simulate_transaction(ExecuteTransactionRequest) -> ExecuteTransactionResponse;

    pub fn execute_raw_transaction(ExecuteRawTransactionRequest) -> ExecuteTransactionResponse;

    pub fn get_transaction_record(TransactionRecordRequest) -> TransactionRecordResponse;

    pub fn get_account_balance(AccountRequest) -> AccountBalanceResponse;

    pub fn get_account_nonce(AccountRequest) -> AccountNonceResponse;

    pub fn get_account_code(AccountRequest) -> AccountCodeResponse;

    pub fn get_block_by_number(BlockRequest) -> BlockResponse;
}
