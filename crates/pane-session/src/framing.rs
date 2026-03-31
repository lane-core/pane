//! Shared length-prefixed framing used by UnixTransport, SessionSource,
//! and write_message. One implementation, used everywhere.

use std::io::{self, IoSlice, Read, Write};

/// Maximum message size: 16 MB.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Write a length-prefixed message.
///
/// Uses vectored I/O to send the 4-byte length prefix and body in a
/// single syscall where possible, preventing Nagle's algorithm from
/// splitting them into separate TCP segments.
///
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
    let len_bytes = len.to_le_bytes();
    let bufs = &[IoSlice::new(&len_bytes), IoSlice::new(data)];
    let total = 4 + data.len();
    // Vectored write sends both pieces in one syscall — Nagle can't split.
    // Handles the common case (everything written at once) and the rare
    // partial-write case (kernel buffer nearly full).
    let n = writer.write_vectored(bufs)?;
    if n < total {
        if n < 4 {
            writer.write_all(&len_bytes[n..])?;
            writer.write_all(data)?;
        } else {
            writer.write_all(&data[n - 4..])?;
        }
    }
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
