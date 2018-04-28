use hexutil::ParseHexError;
use jsonrpc_core;
use rlp::DecoderError;
use secp256k1;
use sputnikvm::errors::PreExecutionError;
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

impl From<PreExecutionError> for Error {
    fn from(val: PreExecutionError) -> Error {
        Error::CallError
    }
}

impl From<DecoderError> for Error {
    fn from(val: DecoderError) -> Error {
        Error::RlpError
    }
}

impl From<ParseHexError> for Error {
    fn from(val: ParseHexError) -> Error {
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
