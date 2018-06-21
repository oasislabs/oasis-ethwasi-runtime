use ethereum_types::{Address, H256, U256};
use rpc::RPCLogFilter;
use sha3::{Digest, Keccak256};
use std::{collections::HashMap, str::FromStr, sync::Arc, thread};

use super::{util::*, Either, RPCLog};
use ekiden_rpc_client;
use evm_api::LogFilter;

use error::Error;
use evm;
use futures::future::Future;

#[derive(Clone, Debug)]
pub enum Filter {
  PendingTransaction(usize),
  Block(usize),
  Log(LogFilter),
}

pub struct FilterManager {
  client: Arc<evm::Client>,
  filters: HashMap<usize, Filter>,
  unmodified_filters: HashMap<usize, Filter>,
}

impl FilterManager {
  pub fn new(client: Arc<evm::Client>) -> Self {
    FilterManager {
      client,
      filters: HashMap::new(),
      unmodified_filters: HashMap::new(),
    }
  }


    pub fn install_log_filter(&mut self, filter: LogFilter) -> usize {
        let id = self.filters.len();
        self.filters.insert(id, Filter::Log(filter.clone()));
        self.unmodified_filters
            .insert(id, Filter::Log(filter.clone()));
        id
    }

    pub fn install_block_filter(&mut self) -> usize {
        let block_height = self.client
            .get_block_height(true)
            .wait()
            .unwrap()
            .as_usize();

        let id = self.filters.len();
        self.filters.insert(id, Filter::Block(block_height));
        self.unmodified_filters
            .insert(id, Filter::Block(block_height));
        id
    }

    pub fn install_pending_transaction_filter(&mut self) -> usize {
        /*
        let mut client = self.client.lock().unwrap();

        let pending_transactions = state.all_pending_transaction_hashes();
        let id = self.filters.len();
        self.filters.insert(id, Filter::PendingTransaction(pending_transactions.len()));
        self.unmodified_filters.insert(id, Filter::PendingTransaction(pending_transactions.len()));
        id
        */
    0usize
  }

  pub fn uninstall_filter(&mut self, id: usize) {
    self.filters.remove(&id);
    self.unmodified_filters.remove(&id);
  }

  pub fn get_logs(&mut self, id: usize) -> Result<Vec<RPCLog>, Error> {
    /*
        let state = self.state.lock().unwrap();

        let filter = self.unmodified_filters.get(&id).ok_or(Error::NotFound)?;

        match filter {
            &Filter::Log(ref filter) => {
                let ret = get_logs(&state, filter.clone())?;
                Ok(ret)
            },
            _ => Err(Error::NotFound),
        }
        */
    Err(Error::NotImplemented)
  }

  pub fn get_changes(&mut self, id: usize) -> Result<Either<Vec<String>, Vec<RPCLog>>, Error> {
    let filter = self.filters.get_mut(&id).ok_or(Error::NotFound)?;

    match filter {
      &mut Filter::Block(ref mut next_start) => {
        let block_hashes = self
          .client
          .get_latest_block_hashes(U256::from(*next_start))
          .wait()
          .unwrap();
        *next_start += block_hashes.len();
        Ok(Either::Left(
          block_hashes.iter().map(|h| format!("0x{:x}", h)).collect(),
        ))
      }
      /*
            &mut Filter::PendingTransaction(ref mut next_start) => {
                let pending_transactions = state.all_pending_transaction_hashes();
                let mut ret = Vec::new();
                while *next_start < pending_transactions.len() {
                    ret.push(format!("0x{:x}", &pending_transactions[*next_start]));
                    *next_start += 1;
                }
                Ok(Either::Left(ret))
            },
            &mut Filter::Log(ref mut filter) => {
                let ret = get_logs(&state, filter.clone())?;
                filter.from_block = state.block_height() + 1;
                Ok(Either::Right(ret))
            },
            */
      _ => return Err(Error::NotImplemented),
    }
  }
}
