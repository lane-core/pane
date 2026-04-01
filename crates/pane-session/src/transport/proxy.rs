//! Protocol-tracing transport wrapper.
//!
//! A transparent proxy that logs all sent and received messages to a
//! writer. Wraps any [`Transport`] implementation — the session layer
//! above and the real transport below are both unaware of the tracing.
//!
//! # Usage
//!
//! ```ignore
//! use std::fs::File;
//! use pane_session::transport::proxy::ProxyTransport;
//! use pane_session::transport::tcp::TcpTransport;
//!
//! let inner = TcpTransport::from_stream(stream);
//! let log = File::create("/tmp/protocol-trace.log").unwrap();
//! let traced = ProxyTransport::new(inner, log, "client-1");
//! // Use `traced` anywhere you'd use the inner transport
//! ```
//!
//! # Plan 9
//!
//! `iostats` encapsulated a process in a monitored namespace and
//! logged all 9P requests to the outside world. `exportfs -d -f
//! dbgfile` logged all 9P traffic to a file. `ProxyTransport` is
//! the same pattern applied to pane's session layer: insert it
//! between application and compositor to trace every protocol
//! message without modifying either side.

use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use crate::error::SessionError;
use crate::transport::Transport;

/// A transparent transport wrapper that logs all protocol traffic.
///
/// Wraps an inner transport and writes a timestamped log entry for
/// every `send_raw` and `recv_raw` call. The log format is
/// human-readable text, one line per message, suitable for `tail -f`
/// or post-hoc analysis.
///
/// The writer is wrapped in a `Mutex` so that `ProxyTransport` can
/// be constructed with a shared writer (e.g., a single log file for
/// multiple connections). The mutex is uncontended in the common
/// single-connection case.
///
/// # Plan 9
///
/// `iostats` from the names paper — transparent 9P proxy that
/// monitors file operations. `exportfs -d` — log all 9P traffic
/// to a debug file. `ProxyTransport` combines both: transparent
/// wrapping with per-message logging.
pub struct ProxyTransport<T, W> {
    inner: T,
    writer: Mutex<W>,
    epoch: Instant,
    label: String,
}

impl<T, W> ProxyTransport<T, W>
where
    T: Transport,
    W: Write,
{
    /// Wrap a transport with protocol tracing.
    ///
    /// All messages are logged to `writer` with timestamps relative
    /// to the transport's creation time. The `label` identifies this
    /// connection in the log output (e.g., "client-3" or "tcp:10.0.0.1:7070").
    pub fn new(inner: T, writer: W, label: impl Into<String>) -> Self {
        ProxyTransport {
            inner,
            writer: Mutex::new(writer),
            epoch: Instant::now(),
            label: label.into(),
        }
    }

    /// Wrap with a shared writer.
    ///
    /// Use this when multiple `ProxyTransport` instances should log
    /// to the same file. The `Mutex<W>` is shared via the caller
    /// providing a writer that is itself `Clone` (e.g., `Arc<Mutex<File>>`
    /// flattened, or any writer that handles concurrent access).
    pub fn with_label(inner: T, writer: W, label: impl Into<String>) -> Self {
        Self::new(inner, writer, label)
    }

    /// Extract the inner transport, discarding the tracing layer.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Write a log entry. Best-effort — log write failures are
    /// silently ignored (tracing must never break the transport).
    fn log(&self, direction: &str, data: &[u8]) {
        let elapsed = self.epoch.elapsed();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();

        // Build the hex preview: first 64 bytes as hex pairs
        let preview_len = data.len().min(64);
        let hex: String = data[..preview_len]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let truncated = if data.len() > 64 { "..." } else { "" };

        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(
                w,
                "{:>4}.{:03} {} {} {} bytes [{}{}]",
                secs,
                millis,
                self.label,
                direction,
                data.len(),
                hex,
                truncated,
            );
        }
    }
}

impl<T, W> Transport for ProxyTransport<T, W>
where
    T: Transport,
    W: Write,
{
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.log(">>", data);
        self.inner.send_raw(data)
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        let data = self.inner.recv_raw()?;
        self.log("<<", &data);
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Send as S, Recv as R, End, Chan};
    use std::sync::Arc;

    type Proto = S<String, R<u64, End>>;

    #[test]
    fn proxy_logs_send_and_recv() {
        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        let client_transport = crate::transport::memory::MemoryTransport::new(tx1, rx2);
        let server_transport = crate::transport::memory::MemoryTransport::new(tx2, rx1);

        let log_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let log_writer = LogWriter(Arc::clone(&log_buf));

        let mut proxy = ProxyTransport::new(client_transport, log_writer, "test-client");

        // Send some data
        proxy.send_raw(b"hello").unwrap();

        // Server side receives
        let mut server = server_transport;
        let data = server.recv_raw().unwrap();
        assert_eq!(data, b"hello");

        // Server sends reply
        server.send_raw(b"world").unwrap();

        // Client receives through proxy
        let reply = proxy.recv_raw().unwrap();
        assert_eq!(reply, b"world");

        // Check log output
        let log = log_buf.lock().unwrap();
        let log_str = String::from_utf8_lossy(&log);
        assert!(log_str.contains(">>"), "log should contain send marker");
        assert!(log_str.contains("<<"), "log should contain recv marker");
        assert!(log_str.contains("test-client"), "log should contain label");
        assert!(log_str.contains("5 bytes"), "log should contain send size");
    }

    #[test]
    fn proxy_transparent_to_session_types() {
        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        let client_transport = crate::transport::memory::MemoryTransport::new(tx1, rx2);
        let server_transport = crate::transport::memory::MemoryTransport::new(tx2, rx1);

        let log_buf = Vec::<u8>::new();
        let proxy = ProxyTransport::new(client_transport, log_buf, "session-test");

        let client: Chan<Proto, _> = Chan::new(proxy);
        let server: Chan<R<String, S<u64, End>>, _> = Chan::new(server_transport);

        let server_handle = std::thread::spawn(move || {
            let (msg, server) = server.recv().unwrap();
            assert_eq!(msg, "traced message");
            let server = server.send(42u64).unwrap();
            server.close();
        });

        let client = client.send("traced message".to_string()).unwrap();
        let (val, client) = client.recv().unwrap();
        assert_eq!(val, 42);
        client.close();

        server_handle.join().unwrap();
    }

    /// Helper writer that writes to a shared Vec<u8>.
    struct LogWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for LogWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
