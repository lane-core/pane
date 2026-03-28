//! Shared length-prefixed framing used by UnixTransport, SessionSource,
//! and write_message. One implementation, used everywhere.

use std::io::{self, Read, Write};

/// Maximum message size: 16 MB.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Write a length-prefixed message.
/// Returns an error if the payload exceeds MAX_MESSAGE_SIZE or u32::MAX.
pub fn write_framed(writer: &mut impl Write, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", data.len(), MAX_MESSAGE_SIZE),
        ));
    }
    let len: u32 = data.len().try_into().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "message too large for u32 length prefix")
    })?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(data)?;
    writer.flush()
}

/// Read a length-prefixed message (blocking).
pub fn read_framed(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
        ));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}
