use ekiden_core::contract::contract_api;

contract_api! {
    pub fn inject_accounts(Vec<AccountState>) -> ();

    pub fn inject_account_storage(Vec<(Address, H256, H256)>) -> ();

    pub fn debug_null_call(bool) -> ();

    pub fn simulate_transaction(TransactionRequest) -> SimulateTransactionResponse;

    pub fn execute_raw_transaction(Vec<u8>) -> ExecuteTransactionResponse;

    pub fn get_block_height(bool) -> U256;

    pub fn get_latest_block_hashes(U256) -> Vec<H256>;

    pub fn get_transaction(H256) -> Option<Transaction>;

    pub fn get_receipt(H256) -> Option<Receipt>;

    pub fn get_account_balance(Address) -> U256;

    pub fn get_account_nonce(Address) -> U256;

    pub fn get_account_code(Address) -> Option<Vec<u8>>;

    pub fn get_block_hash(BlockId) -> Option<H256>;

    pub fn get_block(BlockId) -> Option<Vec<u8>>;

    pub fn get_storage_at((Address, H256)) -> H256;

    pub fn get_logs(Filter) -> Vec<Log>;
}
