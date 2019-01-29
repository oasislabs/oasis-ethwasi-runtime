mod confidential_ctx;
mod crypto;
pub mod key_manager;

pub use self::{confidential_ctx::ConfidentialCtx, key_manager::KeyManagerClient};

/// 4-byte prefix prepended to all confidential contract bytecode: 0x00656e63.
pub const CONFIDENTIAL_PREFIX: &'static [u8; 4] = b"\0enc";

/// Returns true if the payload has the confidential prefix.
pub fn has_confidential_prefix(data: &[u8]) -> bool {
    if data.len() < CONFIDENTIAL_PREFIX.len() {
        return false;
    }
    let prefix = &data[..CONFIDENTIAL_PREFIX.len()];
    return prefix == CONFIDENTIAL_PREFIX;
}
