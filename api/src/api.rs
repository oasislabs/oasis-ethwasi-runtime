use ekiden_core::runtime::runtime_api;

runtime_api! {
    pub fn simulate_transaction(TransactionRequest) -> SimulateTransactionResponse;

    pub fn execute_raw_transaction(Vec<u8>) -> ExecuteTransactionResponse;

    pub fn get_block_height(bool) -> U256;

    pub fn get_transaction(H256) -> Option<Transaction>;

    pub fn get_receipt(H256) -> Option<Receipt>;

    pub fn get_account_balance(Address) -> U256;

    pub fn get_account_nonce(Address) -> U256;

    pub fn get_account_code(Address) -> Option<Vec<u8>>;

    pub fn get_block_hash(BlockId) -> Option<H256>;

    pub fn get_block(BlockId) -> Option<Vec<u8>>;

    pub fn get_storage_at((Address, H256)) -> H256;

    pub fn get_storage_expiry(Address) -> u64;

    pub fn get_logs(Filter) -> Vec<Log>;
}
