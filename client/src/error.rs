use hex;
use jsonrpc_core;
use rlp::DecoderError;
use rustc_hex;
use secp256k1;
use std::num::ParseIntError;

#[derive(Debug)]
pub enum Error {
  InvalidParams,
  HexError,
  IntError,
  UnsupportedTrieQuery,
  ECDSAError,
  NotFound,
  RlpError,
  CallError,
  UnknownSourceMapJump,
  NotImplemented,
  TODO,
}

impl From<DecoderError> for Error {
  fn from(val: DecoderError) -> Error {
    Error::RlpError
  }
}

impl From<rustc_hex::FromHexError> for Error {
  fn from(val: rustc_hex::FromHexError) -> Error {
    Error::HexError
  }
}

impl From<hex::FromHexError> for Error {
  fn from(val: hex::FromHexError) -> Error {
    Error::HexError
  }
}

impl From<ParseIntError> for Error {
  fn from(val: ParseIntError) -> Error {
    Error::IntError
  }
}

impl From<secp256k1::Error> for Error {
  fn from(val: secp256k1::Error) -> Error {
    Error::ECDSAError
  }
}

impl Into<jsonrpc_core::Error> for Error {
  fn into(self) -> jsonrpc_core::Error {
    jsonrpc_core::Error::invalid_request()
  }
}
