// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Trie interface and implementation.
// extern crate rand;
extern crate elastic_array;
extern crate ethcore_bytes as bytes;
extern crate ethereum_types;
extern crate hashdb;
extern crate keccak_hash as keccak;
extern crate memorydb;
extern crate rlp;
// extern crate ethcore_logger;

#[cfg(test)]
extern crate trie_standardmap as standardmap;

extern crate log;

use ethereum_types::H256;
use hashdb::DBValue;
use keccak::KECCAK_NULL_RLP;
use std::{error, fmt};

pub mod node;
pub mod recorder;
pub mod triedb;

mod lookup;
mod nibbleslice;
mod nibblevec;

pub use self::{
    recorder::Recorder,
    triedb::{TrieDB, TrieDBIterator},
};

/// Trie Errors.
///
/// These borrow the data within them to avoid excessive copying on every
/// trie operation.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrieError {
    /// Attempted to create a trie with a state root not in the DB.
    InvalidStateRoot(H256),
    /// Trie item not found in the database,
    IncompleteDatabase(H256),
    /// Corrupt Trie item
    DecoderError(rlp::DecoderError),
}

impl fmt::Display for TrieError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TrieError::InvalidStateRoot(ref root) => write!(f, "Invalid state root: {}", root),
            TrieError::IncompleteDatabase(ref missing) => {
                write!(f, "Database missing expected key: {}", missing)
            }
            TrieError::DecoderError(ref err) => write!(f, "Decoding failed with {}", err),
        }
    }
}

impl error::Error for TrieError {
    fn description(&self) -> &str {
        match *self {
            TrieError::InvalidStateRoot(_) => "Invalid state root",
            TrieError::IncompleteDatabase(_) => "Incomplete database",
            TrieError::DecoderError(ref e) => e.description(),
        }
    }
}

impl From<rlp::DecoderError> for Box<TrieError> {
    fn from(e: rlp::DecoderError) -> Self {
        Box::new(TrieError::DecoderError(e))
    }
}

/// Trie result type. Boxed to avoid copying around extra space for `H256`s on successful queries.
pub type Result<T> = ::std::result::Result<T, Box<TrieError>>;

/// Trie-Item type.
pub type TrieItem<'a> = Result<(Vec<u8>, DBValue)>;

/// Description of what kind of query will be made to the trie.
///
/// This is implemented for any &mut recorder (where the query will return
/// a DBValue), any function taking raw bytes (where no recording will be made),
/// or any tuple of (&mut Recorder, FnOnce(&[u8]))
pub trait Query {
    /// Output item.
    type Item;

    /// Decode a byte-slice into the desired item.
    fn decode(self, &[u8]) -> Self::Item;

    /// Record that a node has been passed through.
    fn record(&mut self, &H256, &[u8], u32) {}
}

impl<'a> Query for &'a mut Recorder {
    type Item = DBValue;

    fn decode(self, value: &[u8]) -> DBValue {
        DBValue::from_slice(value)
    }
    fn record(&mut self, hash: &H256, data: &[u8], depth: u32) {
        (&mut **self).record(hash, data, depth);
    }
}

impl<F, T> Query for F
where
    F: for<'a> FnOnce(&'a [u8]) -> T,
{
    type Item = T;

    fn decode(self, value: &[u8]) -> T {
        (self)(value)
    }
}

impl<'a, F, T> Query for (&'a mut Recorder, F)
where
    F: FnOnce(&[u8]) -> T,
{
    type Item = T;

    fn decode(self, value: &[u8]) -> T {
        (self.1)(value)
    }
    fn record(&mut self, hash: &H256, data: &[u8], depth: u32) {
        self.0.record(hash, data, depth)
    }
}

/// A key-value datastore implemented as a database-backed modified Merkle tree.
pub trait Trie {
    /// Return the root of the trie.
    fn root(&self) -> &H256;

    /// Is the trie empty?
    fn is_empty(&self) -> bool {
        *self.root() == KECCAK_NULL_RLP
    }

    /// Does the trie contain a given key?
    fn contains(&self, key: &[u8]) -> Result<bool> {
        self.get(key).map(|x| x.is_some())
    }

    /// What is the value of the given key in this trie?
    fn get<'a, 'key>(&'a self, key: &'key [u8]) -> Result<Option<DBValue>>
    where
        'a: 'key,
    {
        self.get_with(key, DBValue::from_slice)
    }

    /// Search for the key with the given query parameter. See the docs of the `Query`
    /// trait for more details.
    fn get_with<'a, 'key, Q: Query>(&'a self, key: &'key [u8], query: Q) -> Result<Option<Q::Item>>
    where
        'a: 'key;

    /// Returns a depth-first iterator over the elements of trie.
    fn iter<'a>(&'a self) -> Result<Box<TrieIterator<Item = TrieItem> + 'a>>;
}

/// A key-value datastore implemented as a database-backed modified Merkle tree.
pub trait TrieMut {
    /// Return the root of the trie.
    fn root(&mut self) -> &H256;

    /// Is the trie empty?
    fn is_empty(&self) -> bool;

    /// Does the trie contain a given key?
    fn contains(&self, key: &[u8]) -> Result<bool> {
        self.get(key).map(|x| x.is_some())
    }

    /// What is the value of the given key in this trie?
    fn get<'a, 'key>(&'a self, key: &'key [u8]) -> Result<Option<DBValue>>
    where
        'a: 'key;

    /// Insert a `key`/`value` pair into the trie. An empty value is equivalent to removing
    /// `key` from the trie. Returns the old value associated with this key, if it existed.
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<Option<DBValue>>;

    /// Remove a `key` from the trie. Equivalent to making it equal to the empty
    /// value. Returns the old value associated with this key, if it existed.
    fn remove(&mut self, key: &[u8]) -> Result<Option<DBValue>>;
}

/// A trie iterator that also supports random access.
pub trait TrieIterator: Iterator {
    /// Position the iterator on the first element with key > `key`
    fn seek(&mut self, key: &[u8]) -> Result<()>;
}
