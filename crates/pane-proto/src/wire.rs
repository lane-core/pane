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
/// Returns an error if the payload exceeds u32::MAX bytes.
pub fn frame(payload: &[u8]) -> Result<Vec<u8>, postcard::Error> {
    let len: u32 = payload
        .len()
        .try_into()
        .map_err(|_| postcard::Error::SerializeBufferFull)?;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Read the length prefix from a framed message.
/// Returns None if the buffer is too short.
pub fn frame_length(buf: &[u8]) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

/// Maximum message size (16 MB). Prevents malicious or corrupt length
/// prefixes from causing unbounded allocation.
///
/// Note: pane-session has its own copy of write_framed/read_framed in
/// framing.rs. Both must stay in sync. The duplication exists because
/// pane-session depends on pane-proto (not vice versa), so pane-proto
/// can't call pane-session's framing. This is acceptable for 15 lines
/// of logic — a shared crate would be over-engineering.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Write a length-prefixed message to a writer.
/// Returns an error if the payload exceeds MAX_MESSAGE_SIZE or u32::MAX.
pub fn write_framed(writer: &mut impl std::io::Write, data: &[u8]) -> std::io::Result<()> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", data.len(), MAX_MESSAGE_SIZE),
        ));
    }
    let len: u32 = data.len().try_into().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "message too large for u32 length prefix")
    })?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(data)?;
    writer.flush()
}

/// Read a length-prefixed message from a reader.
/// Returns Err on disconnect, oversized message, or I/O error.
pub fn read_framed(reader: &mut impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
        ));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}
