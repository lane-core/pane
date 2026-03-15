use serde::{de::DeserializeOwned, Serialize};

/// Serialize a value to postcard bytes.
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(value)
}

/// Deserialize a value from postcard bytes.
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

/// Encode a message with a 4-byte little-endian length prefix.
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// Read the length prefix from a framed message.
/// Returns None if the buffer is too short.
pub fn frame_length(buf: &[u8]) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}
