//! Bridge: connects par's in-process session channels to IPC.
//!
//! Two-phase connection:
//!   Phase 1 (connect): verify the transport is alive. Returns Result.
//!   Phase 2 (handshake): par drives the exchange over the verified
//!     transport via FrameCodec framing. If the transport dies
//!     mid-handshake, the session is aborted (panic). This is the
//!     rare case — the common failure (server not running) is caught
//!     in Phase 1.
//!
//! Architecture:
//!   Handler ←→ par oneshot ←→ Bridge thread ←→ FrameCodec ←→ wire
//!
//! The handler uses par's Send::send() and Recv::recv() directly.
//! The bridge thread serializes between par's channels and the
//! transport: CBOR for the handshake (Hello/Welcome), postcard +
//! FrameCodec for data-plane frames after handshake.
//!
//! After the handshake succeeds, the bridge thread splits the
//! transport and spawns a reader loop (feeding LooperMessages to
//! the looper) and a writer loop (draining outbound frames from
//! ServiceHandle/PaneBuilder). Two threads per connection.
//!
//! Design heritage: BeOS split connection into find_port (returned
//! port_id, negative on error; headers/os/kernel/OS.h:134) and the
//! AS_CREATE_APP exchange (debugger on failure;
//! src/kits/app/Application.cpp:1432). Plan 9's mount() returned
//! -1 on unreachable (bind(2), reference/plan9/man/2/bind:227);
//! the Tversion...Tattach sequence was the handshake over a
//! verified fd.

use crate::frame::{Frame, FrameCodec};
use crate::handshake::{ClientHandshake, Hello, Rejection, ServerHandshake, Welcome};
use crate::transport::{ConnectError, Transport};
use pane_proto::control::ControlMessage;
use par::exchange::{Recv as ParRecv, Send as ParSend};
use par::Session;

/// Unified message type from the reader thread to the looper.
/// Single channel preserves causal ordering between control and
/// service frames (round 1 consensus: BLooper had one port).
///
/// Design heritage: BeOS BLooper had one port, one queue — a single
/// message stream with interleaved types, dispatched sequentially
/// (src/kits/app/Looper.cpp:1162, MessageFromPort). Plan 9 devmnt
/// similarly used one fd per mount for all message types (devmnt.c:803).
/// A single LooperMessage enum over one mpsc channel preserves
/// this causal ordering invariant.
#[derive(Debug)]
pub enum LooperMessage {
    /// Control channel (service 0): lifecycle, service negotiation.
    Control(ControlMessage),
    /// Service frame: session_id + raw payload (ServiceFrame bytes).
    /// The looper parses ServiceFrame and routes by variant.
    Service { session_id: u16, payload: Vec<u8> },
    /// Local revocation: a ServiceHandle was dropped, releasing
    /// interest in the given session. Posted to the looper's input
    /// channel so revocation participates in batch ordering (D8).
    /// Phase 4 (ctl writes) sends the wire RevokeInterest; phase 5
    /// suppresses stale frames for the revoked session (H3).
    ///
    /// Currently not wired from ServiceHandle::Drop — ServiceHandle
    /// lacks access to the looper's input channel (it only holds
    /// write_tx). The variant is exercised by batch-side machinery
    /// and will be connected when ConnectionSource (C4) restructures
    /// the channel topology. Until then, ServiceHandle::Drop sends
    /// RevokeInterest directly via try_send on the data channel.
    LocalRevoke { session_id: u16 },
}

/// Write-side message: (service_id, payload bytes).
pub type WriteMessage = (u16, Vec<u8>);

/// Write channel capacity (bounded backpressure).
///
/// When the channel is full, the sender (handler) blocks until the
/// writer thread drains frames to the transport. Per-pane threading
/// isolates the stall — other panes are unaffected.
///
/// Design heritage: BeOS BLooper used B_LOOPER_PORT_DEFAULT_CAPACITY
/// of 200 (src/kits/app/Looper.cpp). Plan 9's devmnt MAXRPC was 32.
/// 128 is a middle ground — enough for burst absorption, small enough
/// to cap memory under sustained overload.
pub const WRITE_CHANNEL_CAPACITY: usize = 128;

/// Default max_message_size for the handshake phase.
/// Renegotiated during the handshake — the Welcome carries the
/// agreed value for the active phase.
pub const HANDSHAKE_MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Phase 1: verify a transport is alive.
///
/// Returns the transport on success, ConnectError on failure.
/// No par involved — this is where "server not running" surfaces
/// as a Result, before the session-typed handshake begins.
pub fn verify_transport<T: Transport>(transport: T) -> Result<T, ConnectError> {
    // Real implementations would send a probe/ping here.
    // MemoryTransport is always connected.
    Ok(transport)
}

/// Phase 2: client-side handshake over a verified transport.
///
/// Returns the handler's par session endpoint. A bridge thread
/// handles serialization between par's channels and the transport
/// using FrameCodec on the Control channel.
///
/// If the transport dies mid-handshake, the bridge thread panics,
/// its par endpoint drops, and the handler's next recv() panics
/// ("sender dropped"). This aborts the session.
pub fn bridge_client_handshake(mut transport: impl Transport) -> ClientHandshake {
    ParSend::fork_sync(move |server: ServerHandshake| {
        std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            let (hello, server): (Hello, _) = futures::executor::block_on(server.recv());

            // Write Hello as a framed Control message (service 0).
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&hello, &mut bytes)
                .expect("bridge: Hello serialization failed");
            codec
                .write_frame(&mut transport, 0, &bytes)
                .expect("bridge: Hello write failed");

            // Read the server's decision from a framed Control message.
            let frame = codec
                .read_frame(&mut transport)
                .expect("bridge: handshake response read failed");
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                Frame::Abort => panic!("bridge: server sent ProtocolAbort during handshake"),
                other => panic!("bridge: unexpected frame during handshake: {other:?}"),
            };
            let decision: Result<Welcome, Rejection> =
                ciborium::de::from_reader(payload.as_slice())
                    .expect("bridge: handshake response deserialization failed");

            server.send1(decision);
        });
    })
}

/// Phase 2: server-side handshake over a verified transport.
pub fn bridge_server_handshake(mut transport: impl Transport) -> ServerHandshake {
    ParRecv::fork_sync(move |client: ClientHandshake| {
        std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            // Read Hello from a framed Control message.
            let frame = codec
                .read_frame(&mut transport)
                .expect("bridge: Hello read failed");
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                Frame::Abort => panic!("bridge: client sent ProtocolAbort during handshake"),
                other => panic!("bridge: unexpected frame during handshake: {other:?}"),
            };
            let hello: Hello = ciborium::de::from_reader(payload.as_slice())
                .expect("bridge: Hello deserialization failed");

            let client = client.send(hello);

            let (decision, _): (Result<Welcome, Rejection>, _) =
                futures::executor::block_on(client.recv());

            // Write the decision as a framed Control message.
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&decision, &mut bytes)
                .expect("bridge: handshake response serialization failed");
            codec
                .write_frame(&mut transport, 0, &bytes)
                .expect("bridge: handshake response write failed");
        });
    })
}

/// Convenience: Phase 1 + Phase 2 for the client side.
pub fn connect_client(transport: impl Transport) -> Result<ClientHandshake, ConnectError> {
    let transport = verify_transport(transport)?;
    Ok(bridge_client_handshake(transport))
}

/// Convenience: Phase 1 + Phase 2 for the server side.
pub fn connect_server(transport: impl Transport) -> Result<ServerHandshake, ConnectError> {
    let transport = verify_transport(transport)?;
    Ok(bridge_server_handshake(transport))
}

/// Connect as client, perform handshake, and start reader + writer loops.
///
/// On success, returns the Welcome, a Receiver that delivers
/// LooperMessages from the server, and a write channel for outbound
/// framing (ServiceHandle/PaneBuilder). The transport is split:
/// read half stays in the reader thread, write half in the writer
/// thread.
///
/// Design heritage: Plan 9's mount(2) used a single fd; the kernel
/// split internally. pane splits explicitly for Rust ownership.
pub fn connect_and_run(
    hello: Hello,
    transport: impl crate::transport::TransportSplit,
) -> Result<ClientConnection, ConnectError> {
    let transport = verify_transport(transport)?;

    // The bridge thread performs the handshake (on the unsplit
    // transport), then splits and spawns reader + writer threads.
    // We get the result back via a oneshot-like mpsc channel.
    let (result_tx, result_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        run_client_bridge(transport, hello, result_tx);
    });

    result_rx.recv().map_err(|_| {
        ConnectError::Transport(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "bridge thread died during handshake",
        ))
    })?
}

/// Accept a client connection on the server side.
///
/// Reads the Hello, calls the `decide` closure to produce a
/// decision, sends the response, and if accepted, starts a reader
/// loop feeding ControlMessages back through a channel.
pub fn accept_and_run(
    transport: impl Transport,
    decide: impl FnOnce(Hello) -> Result<Welcome, Rejection> + Send + 'static,
) -> Result<ServerReader, ConnectError> {
    let transport = verify_transport(transport)?;

    let (result_tx, result_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        run_server_bridge(transport, decide, result_tx);
    });

    result_rx.recv().map_err(|_| {
        ConnectError::Transport(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "bridge thread died during handshake",
        ))
    })?
}

/// Result of a successful client connection.
///
/// Bidirectional: holds both the read channel (rx, for the looper)
/// and the write channel (write_tx, for ServiceHandle/PaneBuilder).
/// The bridge spawns a reader thread and a writer thread; the
/// caller interacts only through channels.
///
/// Intentionally distinct from ServerReader, which is read-only.
/// The asymmetry reflects the two-thread architecture: the client
/// bridge owns both reader and writer threads, while the server
/// bridge hands the writer to the ProtocolServer actor (which
/// manages writes centrally via WriteHandle).
#[derive(Debug)]
pub struct ClientConnection {
    pub welcome: Welcome,
    pub rx: std::sync::mpsc::Receiver<LooperMessage>,
    pub write_tx: std::sync::mpsc::SyncSender<WriteMessage>,
}

/// Result of a successful server-side accept. Read-only — the
/// server bridge does not expose a write channel here because
/// the ProtocolServer actor owns all write handles centrally
/// (via WriteHandle in server.rs). See ClientConnection for the
/// bidirectional client-side equivalent.
#[derive(Debug)]
pub struct ServerReader {
    pub hello: Hello,
    pub welcome: Welcome,
    pub rx: std::sync::mpsc::Receiver<LooperMessage>,
}

/// Client bridge thread: handshake on unsplit transport, then split
/// into reader + writer threads for the active phase.
fn run_client_bridge(
    mut transport: impl crate::transport::TransportSplit,
    hello: Hello,
    result_tx: std::sync::mpsc::Sender<Result<ClientConnection, ConnectError>>,
) {
    let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

    // -- Handshake: write Hello, read Welcome (before split) --

    // Handshake uses CBOR (self-describing) for forward compatibility:
    // new fields with #[serde(default)] deserialize from old payloads.
    // Data-plane frames after handshake stay postcard (compact, positional).
    //
    // Design heritage: Haiku converged on the same two-phase format split —
    // BMessage (self-describing) for AS_GET_DESKTOP handshake, link protocol
    // (positional binary) for data plane (src/kits/app/Application.cpp:1402,
    // headers/private/app/LinkSender.h:36-40).
    let bytes = {
        let mut buf = Vec::new();
        match ciborium::ser::into_writer(&hello, &mut buf) {
            Ok(()) => buf,
            Err(e) => {
                let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))));
                return;
            }
        }
    };
    if let Err(e) = codec.write_frame(&mut transport, 0, &bytes) {
        let _ = result_tx.send(Err(ConnectError::Transport(e)));
        return;
    }

    let frame = match codec.read_frame(&mut transport) {
        Ok(f) => f,
        Err(e) => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                e.to_string(),
            ))));
            return;
        }
    };
    let payload = match frame {
        Frame::Message {
            service: 0,
            payload,
        } => payload,
        Frame::Abort => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "server sent ProtocolAbort",
            ))));
            return;
        }
        _ => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected frame during handshake",
            ))));
            return;
        }
    };

    let decision: Result<Welcome, Rejection> = match ciborium::de::from_reader(payload.as_slice()) {
        Ok(d) => d,
        Err(e) => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))));
            return;
        }
    };

    let welcome = match decision {
        Ok(w) => w,
        Err(rejection) => {
            let _ = result_tx.send(Err(ConnectError::Rejected(rejection)));
            return;
        }
    };

    // -- Active phase: split transport, spawn writer thread --

    let max_msg = welcome.max_message_size;
    let (mut reader, mut writer) = transport.into_split();

    // Writer thread: owns write half + its own codec.
    // Bounded channel provides backpressure — when full, the sender
    // (handler) blocks until the writer drains to the transport.
    let (write_tx, write_rx) = std::sync::mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
    let writer_codec = FrameCodec::new(max_msg);
    std::thread::spawn(move || {
        writer_loop(&mut writer, &writer_codec, write_rx);
    });

    // Reader: permissive codec for dynamically assigned session_ids
    // from DeclareInterest.
    let reader_codec = FrameCodec::permissive(max_msg);
    let (msg_tx, msg_rx) = std::sync::mpsc::channel();

    // Signal success before entering the read loop.
    if result_tx
        .send(Ok(ClientConnection {
            welcome: welcome.clone(),
            rx: msg_rx,
            write_tx,
        }))
        .is_err()
    {
        return; // Caller dropped
    }

    // This thread becomes the reader loop.
    reader_loop(&mut reader, &reader_codec, msg_tx);
}

/// Server bridge thread: read Hello, decide, send response, then reader loop.
fn run_server_bridge(
    mut transport: impl Transport,
    decide: impl FnOnce(Hello) -> Result<Welcome, Rejection>,
    result_tx: std::sync::mpsc::Sender<Result<ServerReader, ConnectError>>,
) {
    let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

    // Read Hello from a framed Control message.
    let frame = match codec.read_frame(&mut transport) {
        Ok(f) => f,
        Err(e) => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                e.to_string(),
            ))));
            return;
        }
    };
    let payload = match frame {
        Frame::Message {
            service: 0,
            payload,
        } => payload,
        Frame::Abort => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "client sent ProtocolAbort",
            ))));
            return;
        }
        _ => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected frame during handshake",
            ))));
            return;
        }
    };

    let hello: Hello = match ciborium::de::from_reader(payload.as_slice()) {
        Ok(h) => h,
        Err(e) => {
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))));
            return;
        }
    };

    // Let the caller decide whether to accept.
    let decision = decide(hello.clone());

    // Write the decision as a framed Control message.
    let bytes = {
        let mut buf = Vec::new();
        match ciborium::ser::into_writer(&decision, &mut buf) {
            Ok(()) => buf,
            Err(e) => {
                let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))));
                return;
            }
        }
    };
    if let Err(e) = codec.write_frame(&mut transport, 0, &bytes) {
        let _ = result_tx.send(Err(ConnectError::Transport(e)));
        return;
    }

    let welcome = match decision {
        Ok(w) => w,
        Err(_rejection) => {
            // Server rejected — no reader loop needed.
            // The result_tx will be dropped, signaling the caller
            // via recv() returning Err. But we should send a proper error.
            let _ = result_tx.send(Err(ConnectError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "server rejected the handshake",
            ))));
            return;
        }
    };

    // Handshake succeeded. Permissive codec for the reader loop
    // (same rationale as client side).
    let reader_codec = FrameCodec::permissive(welcome.max_message_size);

    let (msg_tx, msg_rx) = std::sync::mpsc::channel();

    if result_tx
        .send(Ok(ServerReader {
            hello: hello.clone(),
            welcome: welcome.clone(),
            rx: msg_rx,
        }))
        .is_err()
    {
        return;
    }

    reader_loop(&mut transport, &reader_codec, msg_tx);
}

/// Writer thread: drains the write channel and frames each message
/// onto the transport. Exits when the channel closes or a write fails.
///
/// Deadlock-free by construction: the writer thread only blocks on
/// mpsc::recv (memory-to-memory) and write (transport). It never
/// acquires locks or reads from the transport. DAG topology per
/// DLfActRiS Theorem 5.4.
fn writer_loop(
    writer: &mut impl std::io::Write,
    codec: &FrameCodec,
    write_rx: std::sync::mpsc::Receiver<WriteMessage>,
) {
    while let Ok((service, payload)) = write_rx.recv() {
        if codec.write_frame(writer, service, &payload).is_err() {
            break;
        }
    }
}

/// Active-phase reader loop. Reads frames, demuxes by service,
/// and forwards messages to the looper as LooperMessage variants.
///
/// Exits when:
/// - The transport fails (peer disconnected, broken pipe)
/// - The peer sends ProtocolAbort
/// - The msg_tx receiver is dropped (looper shut down)
fn reader_loop(
    transport: &mut impl std::io::Read,
    codec: &FrameCodec,
    msg_tx: std::sync::mpsc::Sender<LooperMessage>,
) {
    loop {
        match codec.read_frame(transport) {
            Ok(Frame::Message {
                service: 0,
                payload,
            }) => {
                let msg: ControlMessage = match postcard::from_bytes(&payload) {
                    Ok(m) => m,
                    Err(_) => {
                        // Deserialization failure on Control — protocol
                        // violation, connection is untrustworthy.
                        break;
                    }
                };
                if msg_tx.send(LooperMessage::Control(msg)).is_err() {
                    // Looper dropped its receiver — shut down.
                    break;
                }
            }
            Ok(Frame::Message { service, payload }) => {
                // Service frame — forward with session_id tag.
                // Looper parses ServiceFrame and routes by variant.
                if msg_tx
                    .send(LooperMessage::Service {
                        session_id: service,
                        payload,
                    })
                    .is_err()
                {
                    break;
                }
            }
            Ok(Frame::Abort) => {
                // Peer sent ProtocolAbort.
                break;
            }
            Err(_) => {
                // Transport error or protocol violation.
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::{RejectReason, Rejection};
    use crate::transport::{MemoryTransport, TransportSplit};

    #[test]
    fn writer_loop_forwards_frames() {
        let (mut client, server) = MemoryTransport::pair();
        let (write_tx, write_rx) = std::sync::mpsc::channel();

        let writer_codec = FrameCodec::new(1024);

        // Spawn writer loop on the server-side write half
        let (_, mut s_writer) = server.into_split();
        std::thread::spawn(move || {
            writer_loop(&mut s_writer, &writer_codec, write_rx);
        });

        // Send a frame through the channel
        let payload = postcard::to_allocvec(&ControlMessage::Lifecycle(
            pane_proto::protocols::lifecycle::LifecycleMessage::Ready,
        ))
        .unwrap();
        write_tx.send((0u16, payload.clone())).unwrap();

        // Read it from the other end
        let reader_codec = FrameCodec::permissive(1024);
        let frame = reader_codec.read_frame(&mut client).unwrap();
        match frame {
            Frame::Message {
                service: 0,
                payload: received,
            } => {
                assert_eq!(received, payload);
            }
            other => panic!("expected Control frame, got {other:?}"),
        }

        // Drop sender to terminate writer loop
        drop(write_tx);
    }

    #[test]
    fn connect_and_run_returns_write_channel() {
        use pane_proto::protocols::lifecycle::LifecycleMessage;

        let (ct, mut st) = MemoryTransport::pair();

        let server_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            // Read Hello
            let frame = codec.read_frame(&mut st).unwrap();
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                _ => panic!("expected Control frame"),
            };
            let _hello: Hello = ciborium::de::from_reader(payload.as_slice()).unwrap();

            // Send Welcome (CBOR — handshake phase)
            let decision: Result<Welcome, Rejection> = Ok(Welcome {
                version: 1,
                instance_id: "write-test-server".into(),
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                bindings: vec![],
            });
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&decision, &mut bytes).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Active phase: read what the client sends via write_tx
            let reader_codec = FrameCodec::permissive(16 * 1024 * 1024);
            let frame = reader_codec.read_frame(&mut st).unwrap();
            match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => {
                    let msg: ControlMessage = postcard::from_bytes(&payload).unwrap();
                    assert!(matches!(
                        msg,
                        ControlMessage::Lifecycle(LifecycleMessage::Ready)
                    ));
                }
                other => panic!("expected Control frame, got {other:?}"),
            }

            drop(st);
        });

        let client = connect_and_run(
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![],
            },
            ct,
        )
        .expect("client connect failed");

        // Write through the write channel
        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        let bytes = postcard::to_allocvec(&msg).unwrap();
        client.write_tx.send((0, bytes)).unwrap();

        server_handle.join().unwrap();
    }

    #[test]
    fn two_phase_handshake_roundtrip() {
        let (ct, st) = MemoryTransport::pair();

        let client = connect_client(ct).expect("client connect failed");
        let server = connect_server(st).expect("server connect failed");

        let client = client.send(Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        });

        let (hello, server) = futures::executor::block_on(server.recv());
        assert_eq!(hello.version, 1);

        server.send1(Ok(Welcome {
            version: 1,
            instance_id: "ada-server".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        }));

        let decision = futures::executor::block_on(client.recv1());
        let welcome = decision.expect("expected welcome, got rejection");
        assert_eq!(welcome.instance_id, "ada-server");
    }

    #[test]
    fn handshake_rejection_roundtrip() {
        let (ct, st) = MemoryTransport::pair();

        let client = connect_client(ct).expect("client connect failed");
        let server = connect_server(st).expect("server connect failed");

        let client = client.send(Hello {
            version: 99,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        });

        let (hello, server) = futures::executor::block_on(server.recv());
        assert_eq!(hello.version, 99);

        server.send1(Err(Rejection {
            reason: RejectReason::VersionMismatch,
            message: Some("server requires version 1".into()),
        }));

        let decision = futures::executor::block_on(client.recv1());
        let rejection = decision.expect_err("expected rejection, got welcome");
        assert!(matches!(rejection.reason, RejectReason::VersionMismatch));
        assert_eq!(
            rejection.message.as_deref(),
            Some("server requires version 1")
        );
    }

    #[test]
    fn phase1_accepts_valid_transport() {
        let (ct, _st) = MemoryTransport::pair();
        let result = verify_transport(ct);
        assert!(result.is_ok());
    }

    #[test]
    fn connect_and_run_handshake_success() {
        use pane_proto::protocols::lifecycle::LifecycleMessage;

        let (ct, mut st) = MemoryTransport::pair();

        // Server side: manually perform handshake via FrameCodec,
        // then drop the transport to signal EOF.
        let server_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            // Read Hello
            let frame = codec.read_frame(&mut st).unwrap();
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                _ => panic!("expected Control frame"),
            };
            let hello: Hello = ciborium::de::from_reader(payload.as_slice()).unwrap();
            assert_eq!(hello.version, 1);

            // Send Welcome (CBOR — handshake phase)
            let decision: Result<Welcome, Rejection> = Ok(Welcome {
                version: 1,
                instance_id: "test-server".into(),
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                bindings: vec![],
            });
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&decision, &mut bytes).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Send a Ready lifecycle message (postcard — active phase)
            let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
            let bytes = postcard::to_allocvec(&msg).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Drop transport to signal EOF
            drop(st);
        });

        let client = connect_and_run(
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![],
            },
            ct,
        )
        .expect("client connect failed");

        assert_eq!(client.welcome.instance_id, "test-server");

        // Read the Ready message (wrapped in LooperMessage)
        let msg = client.rx.recv().expect("expected Ready");
        assert!(matches!(
            msg,
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));

        // Channel closes after server drops transport
        server_handle.join().unwrap();
        assert!(client.rx.recv().is_err());
    }

    #[test]
    fn connect_and_run_rejection() {
        let (ct, st) = MemoryTransport::pair();

        std::thread::spawn(move || {
            let _ = accept_and_run(st, |_hello| {
                Err(Rejection {
                    reason: RejectReason::VersionMismatch,
                    message: Some("nope".into()),
                })
            });
        });

        let err = connect_and_run(
            Hello {
                version: 99,
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![],
            },
            ct,
        )
        .expect_err("expected rejection");

        assert!(matches!(err, ConnectError::Rejected(_)));
    }

    #[test]
    fn connect_and_run_receives_control_messages() {
        use pane_proto::protocols::lifecycle::LifecycleMessage;

        let (ct, mut st) = MemoryTransport::pair();

        // Server side: accept, then manually write Control frames
        // on the wire to simulate active-phase messages.
        let server_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            // Read Hello
            let frame = codec.read_frame(&mut st).unwrap();
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                _ => panic!("expected Control frame"),
            };
            let _hello: Hello = ciborium::de::from_reader(payload.as_slice()).unwrap();

            // Send Welcome (CBOR — handshake phase)
            let welcome = Welcome {
                version: 1,
                instance_id: "msg-server".into(),
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                bindings: vec![],
            };
            let decision: Result<Welcome, Rejection> = Ok(welcome);
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&decision, &mut bytes).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Active phase: send lifecycle messages as Control frames (postcard)
            let ready = ControlMessage::Lifecycle(LifecycleMessage::Ready);
            let bytes = postcard::to_allocvec(&ready).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            let close = ControlMessage::Lifecycle(LifecycleMessage::CloseRequested);
            let bytes = postcard::to_allocvec(&close).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Drop transport to signal EOF
            drop(st);
        });

        let client = connect_and_run(
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![],
            },
            ct,
        )
        .expect("client connect failed");

        assert_eq!(client.welcome.instance_id, "msg-server");

        // Read the two lifecycle messages (wrapped in LooperMessage)
        let msg1 = client.rx.recv().expect("expected Ready");
        assert!(matches!(
            msg1,
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));

        let msg2 = client.rx.recv().expect("expected CloseRequested");
        assert!(matches!(
            msg2,
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::CloseRequested))
        ));

        // Channel closes after server drops
        server_handle.join().unwrap();
        assert!(client.rx.recv().is_err());
    }
}
