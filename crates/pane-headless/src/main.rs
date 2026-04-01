//! Headless pane server.
//!
//! Speaks the full pane protocol — session-typed handshake, active-phase
//! messaging, identity forwarding — without rendering. This is the
//! foundational deployment model that the full desktop extends.
//!
//! Accepts both local (unix socket) and remote (TCP) connections.
//! Builds on any unix-like. No smithay, no Wayland, no GPU.
//!
//! # Architecture
//!
//! pane-headless and pane-comp are parallel consumers of pane-server's
//! `ProtocolServer`. pane-server provides the protocol logic;
//! pane-headless provides a headless calloop event loop; pane-comp
//! provides a graphical event loop with smithay. An application
//! connected to pane-headless cannot tell the difference from the
//! protocol's perspective.
//!
//! # Plan 9
//!
//! In Plan 9, the CPU server ran processes and exported its namespace
//! to terminals via `exportfs(4)`. The terminal was just a client
//! with a display. pane-headless is the CPU server: it runs pane
//! protocol logic and exports to any connected client. pane-comp is
//! the terminal: same protocol, plus a screen. `drawterm` connected
//! to CPU servers without being a full Plan 9 machine — analogous
//! to a pane client connecting to pane-headless from any platform.

mod state;

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use calloop::EventLoop;
use anyhow::Result;
use bpaf::Bpaf;
use tracing::{info, warn};

use state::HeadlessState;

/// Headless pane server — protocol without rendering
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
struct Opts {
    /// Unix socket path (default: $XDG_RUNTIME_DIR/pane/compositor.sock)
    #[bpaf(long("socket"), fallback(String::new()))]
    unix_socket: String,

    /// TCP listen address (e.g., 0.0.0.0:7070). Omit to disable TCP.
    #[bpaf(long("tcp"), argument("ADDR"), optional)]
    tcp_listen: Option<String>,

    /// Default geometry columns
    #[bpaf(long, fallback(80u16))]
    cols: u16,

    /// Default geometry rows
    #[bpaf(long, fallback(24u16))]
    rows: u16,

    /// Log level (error, warn, info, debug, trace)
    #[bpaf(long("log"), fallback("info".to_string()))]
    log_level: String,

    /// Write protocol trace to this file (iostats/exportfs -d pattern).
    /// Logs all handshake and active-phase messages with timestamps.
    #[bpaf(long("protocol-trace"), argument("FILE"), optional)]
    protocol_trace: Option<String>,

    /// PEM certificate file for TLS. Requires --tls-key.
    #[bpaf(long("tls-cert"), argument("FILE"), optional)]
    tls_cert: Option<String>,

    /// PEM private key file for TLS. Requires --tls-cert.
    #[bpaf(long("tls-key"), argument("FILE"), optional)]
    tls_key: Option<String>,
}

fn main() {
    let opts = opts().run();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("pane_headless={}", opts.log_level).into()),
        )
        .init();

    info!("pane-headless starting");

    if let Err(e) = run(&opts) {
        eprintln!("pane-headless fatal: {e:?}");
        std::process::exit(1);
    }

    info!("pane-headless shutdown");
}

/// Shared protocol trace writer. Arc<Mutex<File>> allows multiple
/// connections (each on their own handshake thread) to log to the
/// same file concurrently.
type TraceWriter = Arc<Mutex<File>>;

/// Adapter that implements `Write` by forwarding to the shared trace file.
/// Used as the writer for `ProxyTransport` instances.
struct TraceFile(TraceWriter);

impl Write for TraceFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut f) => f.write(buf),
            Err(_) => Ok(buf.len()), // poisoned mutex — silently discard
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.0.lock() {
            Ok(mut f) => f.flush(),
            Err(_) => Ok(()),
        }
    }
}

fn run(opts: &Opts) -> Result<()> {
    let protocol_server = pane_server::ProtocolServer::new_unmanaged();
    let instance_id = protocol_server.instance_id.clone();

    // Open protocol trace file if requested
    let trace_writer: Option<TraceWriter> = opts.protocol_trace.as_ref().map(|path| {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap_or_else(|e| {
                eprintln!("pane-headless: cannot open trace file {}: {}", path, e);
                std::process::exit(1);
            });
        info!("protocol trace: {}", path);
        Arc::new(Mutex::new(file))
    });

    // --- TLS configuration (optional) ---
    //
    // When --tls-cert and --tls-key are both provided, TCP connections
    // are wrapped in TLS before the session-typed handshake. The TLS
    // handshake completes eagerly (blocking) on the spawned thread,
    // then the session-typed handshake runs over the encrypted channel.
    // After both handshakes complete, the TlsServerTransport is
    // unwrapped back to a raw TcpStream for calloop registration —
    // TLS is a handshake-phase concern, not an active-phase one (the
    // active phase uses calloop's non-blocking I/O on the raw fd).
    //
    // # Plan 9
    //
    // Plan 9's exportfs had `-e 'rc4_256 sha1'` for encrypted export
    // (see `reference/plan9/man/4/exportfs`). The encryption wrapped
    // the 9P connection at the transport layer via ssl(3), transparent
    // to the protocol above. pane follows the same layering: TLS wraps
    // TCP below the session types, invisible to the protocol.
    let tls_config: Option<Arc<rustls::ServerConfig>> = match (&opts.tls_cert, &opts.tls_key) {
        (Some(cert_path), Some(key_path)) => {
            let config = load_tls_config(cert_path, key_path)?;
            info!("tls: loaded cert={} key={}", cert_path, key_path);
            Some(Arc::new(config))
        }
        (None, None) => None,
        (Some(_), None) => {
            anyhow::bail!("--tls-cert requires --tls-key");
        }
        (None, Some(_)) => {
            anyhow::bail!("--tls-key requires --tls-cert");
        }
    };

    // Handshake completion — either unix or TCP stream, with identity.
    enum CompletedHandshake {
        Unix(usize, std::os::unix::net::UnixStream),
        Tcp(usize, std::net::TcpStream, Option<pane_proto::protocol::PeerIdentity>),
    }

    // Handshake completion channel — spawned threads send completed
    // handshakes here. The calloop event loop picks them up via a
    // channel event source (event-driven, not polled).
    let (handshake_tx, handshake_rx) = calloop::channel::channel::<CompletedHandshake>();

    let default_geometry = pane_proto::protocol::PaneGeometry {
        width: opts.cols as u32 * 9,  // approximate cell size
        height: opts.rows as u32 * 17,
        cols: opts.cols,
        rows: opts.rows,
    };

    let mut headless_state = HeadlessState {
        server: protocol_server,
        running: true,
        default_geometry,
        trace_writer: trace_writer.clone(),
        trace_epoch: std::time::Instant::now(),
    };

    // --- calloop event loop ---
    let mut event_loop: EventLoop<'_, HeadlessState> =
        EventLoop::try_new().map_err(|e| anyhow::anyhow!("calloop init: {e}"))?;

    let loop_handle = event_loop.handle();

    // --- handshake completion channel (event-driven, not timer-polled) ---
    loop_handle.insert_source(handshake_rx, {
        let loop_handle = loop_handle.clone();
        move |event, _, state: &mut HeadlessState| {
            if let calloop::channel::Event::Msg(completed) = event {
                match completed {
                    CompletedHandshake::Unix(client_id, stream) => {
                        state.register_unix_client(client_id, stream, &loop_handle);
                    }
                    CompletedHandshake::Tcp(client_id, stream, identity) => {
                        state.register_tcp_client(client_id, stream, identity, &loop_handle);
                    }
                }
            }
        }
    }).map_err(|e| anyhow::anyhow!("handshake channel source: {e}"))?;

    // --- unix listener ---
    let unix_path = if opts.unix_socket.is_empty() {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR not set (use --socket to specify)"))?;
        let pane_dir = std::path::PathBuf::from(&runtime_dir).join("pane");
        std::fs::create_dir_all(&pane_dir)?;
        pane_dir.join("compositor.sock")
    } else {
        std::path::PathBuf::from(&opts.unix_socket)
    };

    let _ = std::fs::remove_file(&unix_path);
    let unix_listener = UnixListener::bind(&unix_path)?;
    unix_listener.set_nonblocking(true)?;
    info!("unix: listening on {}", unix_path.display());

    let unix_source = calloop::generic::Generic::new(
        unix_listener,
        calloop::Interest::READ,
        calloop::Mode::Level,
    );

    let unix_hs_tx = handshake_tx.clone();
    let unix_instance_id = instance_id.clone();
    let unix_trace = trace_writer.clone();
    loop_handle.insert_source(unix_source, move |_event, listener, state: &mut HeadlessState| {
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    info!("new unix client connection");
                    // Accepted stream inherits non-blocking from listener;
                    // the handshake thread does blocking I/O.
                    stream.set_nonblocking(false).ok();
                    let client_id = state.server.alloc_client_id();
                    let sender = unix_hs_tx.clone();
                    let iid = unix_instance_id.clone();
                    let trace = unix_trace.clone();

                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
                        if let Some(ref tw) = trace {
                            // Trace handshake via ProxyTransport
                            use pane_session::transport::proxy::ProxyTransport;
                            use pane_session::transport::unix::UnixTransport;
                            use pane_proto::protocol::ConnectionTopology;
                            let label = format!("unix:{}", client_id);
                            let transport = UnixTransport::from_stream(stream);
                            let proxy = ProxyTransport::new(transport, TraceFile(tw.clone()), label);
                            match pane_server::run_server_handshake_generic(
                                proxy,
                                "pane-headless",
                                &iid,
                                ConnectionTopology::Local,
                            ) {
                                Ok(result) => {
                                    let inner = result.transport.into_inner();
                                    let stream = inner.into_stream();
                                    let _ = sender.send(CompletedHandshake::Unix(client_id, stream));
                                }
                                Err(e) => warn!("unix handshake failed: {}", e),
                            }
                        } else {
                            match pane_server::run_server_handshake(stream, &iid) {
                                Ok(stream) => {
                                    let _ = sender.send(CompletedHandshake::Unix(client_id, stream));
                                }
                                Err(e) => warn!("unix handshake failed: {}", e),
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    warn!("unix accept error: {}", e);
                    break;
                }
            }
        }
        Ok(calloop::PostAction::Continue)
    }).map_err(|e| anyhow::anyhow!("unix listener source: {e}"))?;

    // --- TCP listener (optional) ---
    if let Some(ref addr) = opts.tcp_listen {
        let tcp_listener = TcpListener::bind(addr)?;
        tcp_listener.set_nonblocking(true)?;
        if tls_config.is_some() {
            info!("tcp+tls: listening on {}", addr);
        } else {
            info!("tcp: listening on {}", addr);
        }

        let tcp_source = calloop::generic::Generic::new(
            tcp_listener,
            calloop::Interest::READ,
            calloop::Mode::Level,
        );

        let tcp_hs_tx = handshake_tx.clone();
        let tcp_instance_id = instance_id.clone();
        let tcp_trace = trace_writer.clone();
        let tcp_tls = tls_config.clone();
        loop_handle.insert_source(tcp_source, move |_event, listener, state: &mut HeadlessState| {
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        info!("new tcp client connection from {}", addr);
                        stream.set_nonblocking(false).ok();
                        let client_id = state.server.alloc_client_id();
                        let sender = tcp_hs_tx.clone();
                        let iid = tcp_instance_id.clone();
                        let trace = tcp_trace.clone();
                        let tls = tcp_tls.clone();

                        std::thread::spawn(move || {
                            use pane_session::transport::tcp::TcpTransport;
                            use pane_session::transport::proxy::ProxyTransport;
                            use pane_proto::protocol::ConnectionTopology;

                            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));

                            if let Some(ref server_config) = tls {
                                // TLS path: wrap the TCP stream in TLS, then
                                // run the session-typed handshake over the
                                // encrypted channel. ProxyTransport wraps the
                                // TlsServerTransport so tracing sees decrypted
                                // protocol messages, not TLS ciphertext.
                                use pane_session::transport::tls::accept_tls;

                                let tls_transport = match accept_tls(stream, server_config.clone()) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        warn!("tls handshake failed for {}: {}", addr, e);
                                        return;
                                    }
                                };

                                if let Some(ref tw) = trace {
                                    let label = format!("tcp+tls:{}:{}", addr, client_id);
                                    let proxy = ProxyTransport::new(
                                        tls_transport,
                                        TraceFile(tw.clone()),
                                        label,
                                    );
                                    match pane_server::run_server_handshake_generic(
                                        proxy,
                                        "pane-headless",
                                        &iid,
                                        ConnectionTopology::Remote,
                                    ) {
                                        Ok(result) => {
                                            // Unwrap: ProxyTransport → TlsServerTransport →
                                            // StreamOwned → TcpStream. After handshake, the
                                            // raw stream is registered with calloop for
                                            // non-blocking active-phase I/O.
                                            let tcp_stream = result.transport
                                                .into_inner()
                                                .into_stream()
                                                .sock;
                                            info!("tcp+tls handshake complete for client {} (sig: {})",
                                                client_id, result.signature);
                                            let _ = sender.send(CompletedHandshake::Tcp(client_id, tcp_stream, result.identity));
                                        }
                                        Err(e) => warn!("tcp+tls handshake failed: {}", e),
                                    }
                                } else {
                                    match pane_server::run_server_handshake_generic(
                                        tls_transport,
                                        "pane-headless",
                                        &iid,
                                        ConnectionTopology::Remote,
                                    ) {
                                        Ok(result) => {
                                            let tcp_stream = result.transport
                                                .into_stream()
                                                .sock;
                                            info!("tcp+tls handshake complete for client {} (sig: {})",
                                                client_id, result.signature);
                                            let _ = sender.send(CompletedHandshake::Tcp(client_id, tcp_stream, result.identity));
                                        }
                                        Err(e) => warn!("tcp+tls handshake failed: {}", e),
                                    }
                                }
                            } else {
                                // Plaintext TCP path (no TLS configured).
                                let transport = TcpTransport::from_stream(stream);

                                if let Some(ref tw) = trace {
                                    let label = format!("tcp:{}:{}", addr, client_id);
                                    let proxy = ProxyTransport::new(
                                        transport,
                                        TraceFile(tw.clone()),
                                        label,
                                    );
                                    match pane_server::run_server_handshake_generic(
                                        proxy,
                                        "pane-headless",
                                        &iid,
                                        ConnectionTopology::Remote,
                                    ) {
                                        Ok(result) => {
                                            let tcp_stream = result.transport.into_inner().into_stream();
                                            info!("tcp handshake complete for client {} (sig: {})",
                                                client_id, result.signature);
                                            let _ = sender.send(CompletedHandshake::Tcp(client_id, tcp_stream, result.identity));
                                        }
                                        Err(e) => warn!("tcp handshake failed: {}", e),
                                    }
                                } else {
                                    match pane_server::run_server_handshake_generic(
                                        transport,
                                        "pane-headless",
                                        &iid,
                                        ConnectionTopology::Remote,
                                    ) {
                                        Ok(result) => {
                                            let tcp_stream = result.transport.into_stream();
                                            info!("tcp handshake complete for client {} (sig: {})",
                                                client_id, result.signature);
                                            let _ = sender.send(CompletedHandshake::Tcp(client_id, tcp_stream, result.identity));
                                        }
                                        Err(e) => warn!("tcp handshake failed: {}", e),
                                    }
                                }
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        warn!("tcp accept error: {}", e);
                        break;
                    }
                }
            }
            Ok(calloop::PostAction::Continue)
        }).map_err(|e| anyhow::anyhow!("tcp listener source: {e}"))?;
    }

    // --- signal handling ---
    // On SIGINT/SIGTERM, set running = false to exit the loop.
    // calloop's signal source requires the signals crate on some platforms.
    // For now, use ctrlc for portability.
    let loop_signal = event_loop.get_signal();
    ctrlc::set_handler(move || {
        loop_signal.stop();
    }).map_err(|e| anyhow::anyhow!("signal handler: {e}"))?;

    // --- main loop ---
    info!("pane-headless ready (geometry: {}x{}, {}x{} cells)",
        default_geometry.width, default_geometry.height,
        default_geometry.cols, default_geometry.rows);

    while headless_state.running {
        match event_loop.dispatch(None, &mut headless_state) {
            Ok(_) => {}
            Err(calloop::Error::IoError(ref e)) if e.kind() == std::io::ErrorKind::Interrupted => {
                // Signal received — loop will exit via running flag
                headless_state.running = false;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("event loop error: {e}"));
            }
        }
    }

    // Cleanup unix socket
    let _ = std::fs::remove_file(&unix_path);

    Ok(())
}

/// Load a TLS server configuration from PEM certificate and key files.
///
/// The certificate file may contain a chain (leaf + intermediates).
/// The key file must contain exactly one private key (PKCS#8 or
/// PKCS#1/SEC1).
fn load_tls_config(cert_path: &str, key_path: &str) -> Result<rustls::ServerConfig> {
    let cert_file = File::open(cert_path)
        .map_err(|e| anyhow::anyhow!("cannot open TLS cert {}: {}", cert_path, e))?;
    let key_file = File::open(key_path)
        .map_err(|e| anyhow::anyhow!("cannot open TLS key {}: {}", key_path, e))?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("invalid PEM certificate {}: {}", cert_path, e))?;

    if certs.is_empty() {
        anyhow::bail!("no certificates found in {}", cert_path);
    }

    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|e| anyhow::anyhow!("invalid PEM key {}: {}", key_path, e))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {}", key_path))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| anyhow::anyhow!("TLS config error: {}", e))?;

    Ok(config)
}
