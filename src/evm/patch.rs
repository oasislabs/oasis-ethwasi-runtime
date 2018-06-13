use ethereum_types::{Address, U256, U256};
// use sputnikvm::{AccountPatch, Patch, Precompiled};
// use sputnikvm_network_foundation::{ByzantiumPatch as P, StateClearingAccountPatch as AP};

/// Our ByzantiumPatch is identical to sputnikvm's, except that
/// allow_partial_change() is enabled. Mining rewards are broken for sputnikvm's
/// ByzantiumPatch due to this strange implementation choice for complying
/// with EIP-161d (https://github.com/ethereum/EIPs/blob/master/EIPS/eip-161.md)
///
/// We manage persistent account state independently outside sputnikvm, and we
/// explicitly avoid creating accounts with empty (nonce, code, balance) to
/// comply with EIP-161d.

pub struct StateClearingAccountPatch;
impl AccountPatch for StateClearingAccountPatch {
    fn initial_nonce() -> U256 {
        AP::initial_nonce()
    }
    fn initial_create_nonce() -> U256 {
        AP::initial_create_nonce()
    }
    fn empty_considered_exists() -> bool {
        AP::empty_considered_exists()
    }
    fn allow_partial_change() -> bool {
        true
    }
}

pub struct ByzantiumPatch;
impl Patch for ByzantiumPatch {
    type Account = StateClearingAccountPatch;

    fn code_deposit_limit() -> Option<usize> {
        P::code_deposit_limit()
    }
    fn callstack_limit() -> usize {
        P::callstack_limit()
    }
    fn gas_extcode() -> U256 {
        P::gas_extcode()
    }
    fn gas_balance() -> U256 {
        P::gas_balance()
    }
    fn gas_sload() -> U256 {
        P::gas_sload()
    }
    fn gas_suicide() -> U256 {
        P::gas_suicide()
    }
    fn gas_suicide_new_account() -> U256 {
        P::gas_suicide_new_account()
    }
    fn gas_call() -> U256 {
        P::gas_call()
    }
    fn gas_expbyte() -> U256 {
        P::gas_expbyte()
    }
    fn gas_transaction_create() -> U256 {
        P::gas_transaction_create()
    }
    fn force_code_deposit() -> bool {
        P::force_code_deposit()
    }
    fn has_delegate_call() -> bool {
        P::has_delegate_call()
    }
    fn has_static_call() -> bool {
        P::has_static_call()
    }
    fn has_revert() -> bool {
        P::has_revert()
    }
    fn has_return_data() -> bool {
        P::has_return_data()
    }
    fn err_on_call_with_more_gas() -> bool {
        P::err_on_call_with_more_gas()
    }
    fn call_create_l64_after_gas() -> bool {
        P::call_create_l64_after_gas()
    }
    fn memory_limit() -> usize {
        P::memory_limit()
    }
    fn precompileds() -> &'static [(Address, Option<&'static [u8]>, &'static Precompiled)] {
        P::precompileds()
    }
}
