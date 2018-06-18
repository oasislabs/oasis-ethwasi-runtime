use ekiden_core::contract::contract_api;

contract_api! {
    pub fn genesis_block_initialized(bool) -> bool;
    pub fn inject_accounts(Vec<AccountState>) -> ();
    pub fn inject_account_storage(Vec<(Address, U256, M256)>) -> ();
    pub fn init_genesis_block(InitStateRequest) -> ();

    pub fn debug_execute_unsigned_transaction(Transaction) -> H256;

    pub fn simulate_transaction(Transaction) -> SimulateTransactionResponse;

    pub fn execute_raw_transaction(String) -> H256;

    pub fn get_block_height(bool) -> U256;

    pub fn get_latest_block_hashes(U256) -> Vec<H256>;

    pub fn get_transaction_record(H256) -> Option<TransactionRecord>;

    pub fn get_account_balance(Address) -> U256;

    pub fn get_account_nonce(Address) -> U256;

    pub fn get_account_code(Address) -> String;

    pub fn get_block_by_number(BlockRequest) -> Option<Block>;

    pub fn get_storage_at((Address, U256)) -> M256;

    pub fn get_logs(LogFilter) -> Vec<FilteredLog>;
}
