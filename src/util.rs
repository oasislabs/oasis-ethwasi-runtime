use ekiden_core::error::Result;
use hex;

pub fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

pub fn from_hex<S: AsRef<str>>(hex: S) -> Result<Vec<u8>> {
    Ok(hex::decode(strip_0x(hex.as_ref()))?)
}

pub fn to_hex<T: AsRef<Vec<u8>>>(bytes: T) -> String {
    hex::encode(bytes.as_ref())
}
