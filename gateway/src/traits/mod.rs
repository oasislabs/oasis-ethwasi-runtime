//! RPC traits for the client.

#[cfg(feature = "confidential")]
pub mod confidential;
pub mod oasis;

#[cfg(feature = "confidential")]
pub use self::confidential::Confidential;
pub use self::oasis::Oasis;
