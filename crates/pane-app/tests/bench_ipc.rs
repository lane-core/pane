//! IPC performance benchmarks for pane.
//!
//! Measures real throughput and latency over Unix domain sockets,
//! not MemoryTransport. All tests use `#[ignore]` — run with:
//!
//! ```sh
//! cargo test --test bench_ipc -- --ignored --nocapture
//! ```
//!
//! The `bench_all` test runs all four benchmarks sequentially for
//! clean output ordering. Individual tests exist for running one
//! benchmark at a time.
//!
//! Four benchmarks:
//! 1. Raw notification throughput (1 pub, 1 sub, varying message sizes)
//! 2. Request/reply round-trip latency (percentiles)
//! 3. Fan-out throughput (1 pub, N subs)
//! 4. ConnectionSource direct write path vs mpsc channel path

use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_session::server::ProtocolServer;

use serde::{Deserialize, Serialize};

// ── Wire helpers ─────────────────────────────────────────────

fn bench_service_id() -> ServiceId {
    ServiceId::new("com.pane.bench.topic")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum BenchMessage {
    Payload(Vec<u8>),
}

/// Wire helper for UnixStream connections. Blocking I/O for
/// predictable timing — non-blocking is only used where
/// ConnectionSource requires it.
struct SocketConn {
    stream: UnixStream,
    codec: FrameCodec,
}

impl SocketConn {
    fn connect(mut stream: UnixStream, hello: Hello) -> (Self, Welcome) {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&hello, &mut bytes).unwrap();
        codec.write_frame(&mut stream, 0, &bytes).unwrap();

        let frame = codec.read_frame(&mut stream).unwrap();
        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            other => panic!("unexpected frame: {other:?}"),
        };
        let decision: Result<Welcome, Rejection> =
            ciborium::de::from_reader(payload.as_slice()).unwrap();
        let welcome = decision.expect("rejected");

        let mut codec = codec;
        codec.set_max_message_size(welcome.max_message_size);
        (SocketConn { stream, codec }, welcome)
    }

    fn send_control(&mut self, msg: &ControlMessage) {
        let bytes = postcard::to_allocvec(msg).unwrap();
        self.codec.write_frame(&mut self.stream, 0, &bytes).unwrap();
    }

    fn read_control(&mut self) -> ControlMessage {
        let frame = self.codec.read_frame(&mut self.stream).unwrap();
        match frame {
            Frame::Message {
                service: 0,
                payload,
            } => postcard::from_bytes(&payload).unwrap(),
            other => panic!("expected Control, got {other:?}"),
        }
    }

    fn expect_ready(&mut self) {
        match self.read_control() {
            ControlMessage::Lifecycle(
                pane_proto::protocols::lifecycle::LifecycleMessage::Ready,
            ) => {}
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    fn register_session(&mut self, session_id: u16) {
        self.codec.register_service(session_id);
    }

    fn send_service(&mut self, session_id: u16, frame: &ServiceFrame) {
        let bytes = postcard::to_allocvec(frame).unwrap();
        self.codec
            .write_frame(&mut self.stream, session_id, &bytes)
            .unwrap();
    }

    fn read_service(&mut self) -> (u16, ServiceFrame) {
        let frame = self.codec.read_frame(&mut self.stream).unwrap();
        match frame {
            Frame::Message { service, payload } if service != 0 => {
                let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                (service, sf)
            }
            other => panic!("expected service frame, got {other:?}"),
        }
    }
}

fn accept_unix_on_thread(
    server: &Arc<ProtocolServer>,
    stream: UnixStream,
) -> thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    thread::spawn(move || {
        let writer = stream.try_clone().expect("UnixStream::try_clone failed");
        server.accept(stream, writer).expect("accept failed")
    })
}

fn make_hello(provides: Vec<ServiceProvision>) -> Hello {
    Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        max_outstanding_requests: 0,
        interests: vec![],
        provides,
    }
}

fn tag() -> u8 {
    bench_service_id().tag()
}

/// Build a tagged payload of approximately `size` bytes. The wire
/// frame includes the protocol tag byte + postcard envelope, so
/// the actual inner vec is sized to hit the target approximately.
fn make_payload(size: usize) -> Vec<u8> {
    let inner = vec![0xABu8; size];
    let msg = BenchMessage::Payload(inner);
    let msg_bytes = postcard::to_allocvec(&msg).unwrap();
    let mut buf = Vec::with_capacity(1 + msg_bytes.len());
    buf.push(tag());
    buf.extend_from_slice(&msg_bytes);
    buf
}

/// Make a ServiceFrame::Notification with a payload targeting `size` bytes.
fn make_notification(size: usize) -> (ServiceFrame, usize) {
    let payload = make_payload(size);
    let wire_len = payload.len();
    (ServiceFrame::Notification { payload }, wire_len)
}

/// Subscribe a SocketConn to the publisher's topic. Returns
/// (subscriber, sub_session, pub_session, conn_handle).
fn subscribe_socket(
    server: &Arc<ProtocolServer>,
    publisher: &mut SocketConn,
) -> (SocketConn, u16, u16, pane_session::server::ConnectionHandle) {
    let topic_id = bench_service_id();
    let (sub_client, sub_server) = UnixStream::pair().unwrap();
    let accept = accept_unix_on_thread(server, sub_server);
    let (mut sub, _) = SocketConn::connect(sub_client, make_hello(vec![]));
    let conn = accept.join().unwrap();
    sub.expect_ready();

    sub.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });

    let sub_session = match sub.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("subscriber: expected InterestAccepted, got {other:?}"),
    };
    let pub_session = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("publisher: expected InterestAccepted, got {other:?}"),
    };

    sub.register_session(sub_session);
    publisher.register_session(pub_session);

    (sub, sub_session, pub_session, conn)
}

fn format_throughput(count: usize, total_bytes: usize, elapsed: Duration) -> (f64, f64) {
    let secs = elapsed.as_secs_f64();
    let msgs_per_sec = count as f64 / secs;
    let mb_per_sec = total_bytes as f64 / secs / (1024.0 * 1024.0);
    (msgs_per_sec, mb_per_sec)
}

// ── Benchmark implementations ────────────────────────────────

fn run_notification_throughput() {
    const NUM_MSGS: usize = 10_000;
    const WARMUP: usize = 100;
    const SIZES: &[usize] = &[64, 256, 1024, 4096];

    eprintln!();
    eprintln!("=== Notification Throughput (1 pub -> 1 sub, {NUM_MSGS} msgs) ===");

    for &msg_size in SIZES {
        let server = Arc::new(ProtocolServer::new());
        let topic_id = bench_service_id();

        // Publisher
        let (pub_client, pub_server) = UnixStream::pair().unwrap();
        let accept_pub = accept_unix_on_thread(&server, pub_server);
        let (mut publisher, _) = SocketConn::connect(
            pub_client,
            make_hello(vec![ServiceProvision {
                service: topic_id,
                version: 1,
            }]),
        );
        let conn_pub = accept_pub.join().unwrap();
        publisher.expect_ready();

        // Subscriber
        let (mut sub, sub_session, pub_session, conn_sub) =
            subscribe_socket(&server, &mut publisher);

        let (notification, wire_len) = make_notification(msg_size);
        let total_count = WARMUP + NUM_MSGS;

        // Subscriber thread: read all messages
        let sub_handle = thread::spawn(move || {
            for _ in 0..total_count {
                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Notification { .. } => {}
                    other => panic!("expected Notification, got {other:?}"),
                }
            }
            drop(sub);
            conn_sub.wait();
        });

        // Warmup
        for _ in 0..WARMUP {
            publisher.send_service(pub_session, &notification);
        }

        // Timed run
        let start = Instant::now();
        for _ in 0..NUM_MSGS {
            publisher.send_service(pub_session, &notification);
        }
        // Wait for subscriber to finish reading all messages.
        // The elapsed time captures send + routing + receive.
        sub_handle.join().unwrap();
        let elapsed = start.elapsed();

        let (msgs_sec, mb_sec) = format_throughput(NUM_MSGS, NUM_MSGS * wire_len, elapsed);

        eprintln!(
            "  {msg_size:>4}B msgs: {:>9.0} msg/sec  ({:>6.1} MB/sec)",
            msgs_sec, mb_sec,
        );

        drop(publisher);
        conn_pub.wait();
    }
}

fn run_request_reply_latency() {
    const ITERATIONS: usize = 1_000;
    const WARMUP: usize = 100;
    const MSG_SIZE: usize = 64;

    eprintln!();
    eprintln!("=== Request/Reply Latency ({ITERATIONS} round-trips, {MSG_SIZE}B msgs) ===");

    let server = Arc::new(ProtocolServer::new());
    let topic_id = bench_service_id();

    // Provider (server side)
    let (provider_client, provider_server) = UnixStream::pair().unwrap();
    let accept_provider = accept_unix_on_thread(&server, provider_server);
    let (mut provider, _) = SocketConn::connect(
        provider_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_provider = accept_provider.join().unwrap();
    provider.expect_ready();

    // Client
    let (client_client, client_server) = UnixStream::pair().unwrap();
    let accept_client = accept_unix_on_thread(&server, client_server);
    let (mut client, _) = SocketConn::connect(client_client, make_hello(vec![]));
    let conn_client = accept_client.join().unwrap();
    client.expect_ready();

    // DeclareInterest
    client.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });

    let client_session = match client.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("client: expected InterestAccepted, got {other:?}"),
    };
    let provider_session = match provider.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("provider: expected InterestAccepted, got {other:?}"),
    };

    client.register_session(client_session);
    provider.register_session(provider_session);

    let payload = make_payload(MSG_SIZE);
    let total_count = WARMUP + ITERATIONS;

    // Provider thread: echo requests back as replies
    let provider_handle = thread::spawn(move || {
        for _ in 0..total_count {
            let (session, frame) = provider.read_service();
            assert_eq!(session, provider_session);
            match frame {
                ServiceFrame::Request { token, payload } => {
                    provider
                        .send_service(provider_session, &ServiceFrame::Reply { token, payload });
                }
                other => panic!("provider: expected Request, got {other:?}"),
            }
        }
        drop(provider);
        conn_provider.wait();
    });

    // Warmup
    for i in 0..WARMUP {
        let token = i as u64;
        client.send_service(
            client_session,
            &ServiceFrame::Request {
                token,
                payload: payload.clone(),
            },
        );
        let (_, frame) = client.read_service();
        match frame {
            ServiceFrame::Reply { token: t, .. } => assert_eq!(t, token),
            other => panic!("warmup: expected Reply, got {other:?}"),
        }
    }

    // Timed round-trips
    let mut latencies = Vec::with_capacity(ITERATIONS);
    for i in 0..ITERATIONS {
        let token = (WARMUP + i) as u64;
        let start = Instant::now();
        client.send_service(
            client_session,
            &ServiceFrame::Request {
                token,
                payload: payload.clone(),
            },
        );
        let (_, frame) = client.read_service();
        let elapsed = start.elapsed();
        match frame {
            ServiceFrame::Reply { token: t, .. } => assert_eq!(t, token),
            other => panic!("expected Reply, got {other:?}"),
        }
        latencies.push(elapsed);
    }

    provider_handle.join().unwrap();

    // Compute percentiles
    latencies.sort();
    let mean: Duration = latencies.iter().sum::<Duration>() / ITERATIONS as u32;
    let p50 = latencies[ITERATIONS / 2];
    let p95 = latencies[ITERATIONS * 95 / 100];
    let p99 = latencies[ITERATIONS * 99 / 100];

    eprintln!(
        "  mean: {:>7.1}us  P50: {:>7.1}us  P95: {:>7.1}us  P99: {:>7.1}us",
        mean.as_secs_f64() * 1_000_000.0,
        p50.as_secs_f64() * 1_000_000.0,
        p95.as_secs_f64() * 1_000_000.0,
        p99.as_secs_f64() * 1_000_000.0,
    );

    drop(client);
    conn_client.wait();
}

fn run_fan_out() {
    const NUM_MSGS: usize = 1_000;
    const WARMUP: usize = 50;
    const MSG_SIZE: usize = 256;
    const FAN_OUTS: &[usize] = &[10, 50];

    eprintln!();
    eprintln!("=== Fan-out Throughput (1 pub -> N subs, {NUM_MSGS} msgs x N) ===");

    for &num_subs in FAN_OUTS {
        let server = Arc::new(ProtocolServer::new());
        let topic_id = bench_service_id();

        // Publisher
        let (pub_client, pub_server) = UnixStream::pair().unwrap();
        let accept_pub = accept_unix_on_thread(&server, pub_server);
        let (mut publisher, _) = SocketConn::connect(
            pub_client,
            make_hello(vec![ServiceProvision {
                service: topic_id,
                version: 1,
            }]),
        );
        let conn_pub = accept_pub.join().unwrap();
        publisher.expect_ready();

        // Subscribe all
        let mut subs: Vec<(SocketConn, u16, u16, pane_session::server::ConnectionHandle)> =
            Vec::with_capacity(num_subs);
        for _ in 0..num_subs {
            subs.push(subscribe_socket(&server, &mut publisher));
        }

        let pub_sessions: Vec<u16> = subs.iter().map(|s| s.2).collect();
        let (notification, _) = make_notification(MSG_SIZE);
        let total_count = WARMUP + NUM_MSGS;

        // Spawn subscriber reader threads
        let received = Arc::new(AtomicUsize::new(0));
        let mut sub_handles = Vec::new();

        for (mut sub, sub_session, _, conn) in subs {
            let received = Arc::clone(&received);
            sub_handles.push(thread::spawn(move || {
                for _ in 0..total_count {
                    let (session, frame) = sub.read_service();
                    assert_eq!(session, sub_session);
                    match frame {
                        ServiceFrame::Notification { .. } => {
                            received.fetch_add(1, Ordering::Relaxed);
                        }
                        other => panic!("expected Notification, got {other:?}"),
                    }
                }
                drop(sub);
                conn.wait();
            }));
        }

        // Warmup: send to all subscribers
        for _ in 0..WARMUP {
            for &pub_session in &pub_sessions {
                publisher.send_service(pub_session, &notification);
            }
        }

        // Wait for warmup to be received before starting the timer.
        let warmup_total = WARMUP * num_subs;
        while received.load(Ordering::Relaxed) < warmup_total {
            thread::yield_now();
        }

        // Reset counter for the timed portion
        received.store(0, Ordering::Relaxed);

        // Timed fan-out
        let start = Instant::now();
        for _ in 0..NUM_MSGS {
            for &pub_session in &pub_sessions {
                publisher.send_service(pub_session, &notification);
            }
        }

        // Wait for all subscribers to receive all timed messages
        let target = NUM_MSGS * num_subs;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let got = received.load(Ordering::Relaxed);
            if got >= target {
                break;
            }
            if Instant::now() > deadline {
                panic!("fan-out N={num_subs}: timeout -- received {got}/{target}");
            }
            thread::yield_now();
        }
        let elapsed = start.elapsed();

        let total_frames = NUM_MSGS * num_subs;
        let secs = elapsed.as_secs_f64();
        let total_rate = total_frames as f64 / secs;
        let per_sub_rate = total_rate / num_subs as f64;

        eprintln!(
            "  N={num_subs:>2}: {:>9.0} total frames/sec  ({:>7.0}/sub)",
            total_rate, per_sub_rate,
        );

        for h in sub_handles {
            h.join().unwrap();
        }

        drop(publisher);
        conn_pub.wait();
    }
}

fn run_connection_source_write_paths() {
    use pane_app::ConnectionSource;
    use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
    use std::sync::mpsc;

    const NUM_MSGS: usize = 10_000;
    const WARMUP: usize = 100;
    const MSG_SIZE: usize = 256;

    eprintln!();
    eprintln!("=== ConnectionSource Write Paths ({NUM_MSGS} msgs, {MSG_SIZE}B) ===");

    // --- Path A: mpsc channel path (cross-thread sends) ---
    //
    // Architecture: sender thread pushes through bounded mpsc channel,
    // main thread pumps calloop (drains channel -> FrameWriter -> fd),
    // reader thread reads from the other end of the socket.
    let mpsc_result = {
        let (source_stream, mut peer_stream) = UnixStream::pair().unwrap();
        let (write_tx, write_rx) = mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);

        let source = ConnectionSource::new(source_stream, 16 * 1024 * 1024, write_rx, 1).unwrap();

        let total_count = WARMUP + NUM_MSGS;
        let received = Arc::new(AtomicUsize::new(0));
        let received2 = Arc::clone(&received);

        // Bootstrap: peer sends Ready to wake ConnectionSource
        let ready =
            ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready);
        let ready_bytes = postcard::to_allocvec(&ready).unwrap();
        let bootstrap_codec = FrameCodec::new(16 * 1024 * 1024);
        bootstrap_codec
            .write_frame(&mut peer_stream, 0, &ready_bytes)
            .unwrap();

        // Reader thread (blocking)
        let codec_r = FrameCodec::permissive(16 * 1024 * 1024);
        let reader_handle = thread::spawn(move || {
            for _ in 0..total_count {
                match codec_r.read_frame(&mut peer_stream) {
                    Ok(Frame::Message { service: 1, .. }) => {
                        received2.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(other) => panic!("mpsc reader: unexpected frame: {other:?}"),
                    Err(e) => panic!("mpsc reader: read error: {e}"),
                }
            }
        });

        // Build payload once
        let payload = make_payload(MSG_SIZE);
        let sf = ServiceFrame::Notification {
            payload: payload.clone(),
        };
        let frame_bytes = postcard::to_allocvec(&sf).unwrap();

        // Sender thread: push all frames through mpsc channel.
        let sender_handle = thread::spawn({
            let frame_bytes = frame_bytes.clone();
            move || {
                for _ in 0..total_count {
                    write_tx.send((1u16, frame_bytes.clone())).unwrap();
                }
            }
        });

        // Drive calloop on the main thread (simulates looper thread).
        let mut event_loop: calloop::EventLoop<Vec<pane_session::bridge::LooperMessage>> =
            calloop::EventLoop::try_new().unwrap();

        event_loop
            .handle()
            .insert_source(source, |msg, _, state: &mut Vec<_>| {
                state.push(msg);
            })
            .unwrap();

        let mut events = Vec::new();

        // Pump warmup
        while received.load(Ordering::Relaxed) < WARMUP {
            let _ = event_loop.dispatch(Some(Duration::from_millis(1)), &mut events);
        }

        // Timed portion: pump until all messages delivered
        let start = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(30);
        while received.load(Ordering::Relaxed) < total_count {
            if Instant::now() > deadline {
                panic!(
                    "mpsc path: timeout -- got {}/{}",
                    received.load(Ordering::Relaxed),
                    total_count,
                );
            }
            let _ = event_loop.dispatch(Some(Duration::from_millis(0)), &mut events);
        }
        let elapsed = start.elapsed();

        sender_handle.join().unwrap();
        reader_handle.join().unwrap();

        format_throughput(NUM_MSGS, NUM_MSGS * payload.len(), elapsed)
    };

    eprintln!(
        "  mpsc channel: {:>9.0} msg/sec  ({:>6.1} MB/sec)",
        mpsc_result.0, mpsc_result.1,
    );

    // --- Path B: SharedWriter direct path (looper-thread sends) ---
    let direct_result = {
        let (source_stream, mut peer_stream) = UnixStream::pair().unwrap();
        // The direct path still needs a write_rx, but we won't use it.
        let (_write_tx, write_rx) = mpsc::sync_channel::<pane_session::bridge::WriteMessage>(1);

        let source = ConnectionSource::new(source_stream, 16 * 1024 * 1024, write_rx, 2).unwrap();
        let shared_writer = source.shared_writer();

        let total_count = WARMUP + NUM_MSGS;
        let received = Arc::new(AtomicUsize::new(0));
        let received2 = Arc::clone(&received);

        // Bootstrap: peer sends Ready
        let ready =
            ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready);
        let ready_bytes = postcard::to_allocvec(&ready).unwrap();
        let bootstrap_codec = FrameCodec::new(16 * 1024 * 1024);
        bootstrap_codec
            .write_frame(&mut peer_stream, 0, &ready_bytes)
            .unwrap();

        // Reader thread (blocking)
        let codec_r = FrameCodec::permissive(16 * 1024 * 1024);
        let reader_handle = thread::spawn(move || {
            for _ in 0..total_count {
                match codec_r.read_frame(&mut peer_stream) {
                    Ok(Frame::Message { service: 1, .. }) => {
                        received2.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(other) => panic!("direct reader: unexpected frame: {other:?}"),
                    Err(e) => panic!("direct reader: read error: {e}"),
                }
            }
        });

        // Build payload
        let payload = make_payload(MSG_SIZE);
        let sf = ServiceFrame::Notification {
            payload: payload.clone(),
        };
        let frame_bytes = postcard::to_allocvec(&sf).unwrap();

        // Drive calloop -- enqueue through SharedWriter on the event
        // loop thread (simulating looper-thread direct writes).
        let mut event_loop: calloop::EventLoop<Vec<pane_session::bridge::LooperMessage>> =
            calloop::EventLoop::try_new().unwrap();

        event_loop
            .handle()
            .insert_source(source, |msg, _, state: &mut Vec<_>| {
                state.push(msg);
            })
            .unwrap();

        let mut events = Vec::new();

        // Warmup: enqueue in batches + pump
        for _ in 0..WARMUP {
            shared_writer.enqueue(1, &frame_bytes);
        }
        while received.load(Ordering::Relaxed) < WARMUP {
            let _ = event_loop.dispatch(Some(Duration::from_millis(1)), &mut events);
        }

        // Timed portion: enqueue in batches, pump between batches.
        // This simulates a looper that processes a batch of handler
        // dispatches (each enqueuing a frame) then returns to the
        // calloop event loop to flush.
        let start = Instant::now();
        const BATCH: usize = 64;
        let mut enqueued = 0;
        while enqueued < NUM_MSGS {
            let batch_end = (enqueued + BATCH).min(NUM_MSGS);
            for _ in enqueued..batch_end {
                shared_writer.enqueue(1, &frame_bytes);
            }
            enqueued = batch_end;
            let _ = event_loop.dispatch(Some(Duration::from_millis(0)), &mut events);
        }

        // Drain remaining
        let drain_deadline = Instant::now() + Duration::from_secs(10);
        while received.load(Ordering::Relaxed) < total_count {
            if Instant::now() > drain_deadline {
                panic!(
                    "direct path: timeout -- got {}/{}",
                    received.load(Ordering::Relaxed),
                    total_count,
                );
            }
            let _ = event_loop.dispatch(Some(Duration::from_millis(1)), &mut events);
        }
        let elapsed = start.elapsed();

        reader_handle.join().unwrap();

        format_throughput(NUM_MSGS, NUM_MSGS * payload.len(), elapsed)
    };

    eprintln!(
        "  SharedWriter: {:>9.0} msg/sec  ({:>6.1} MB/sec)",
        direct_result.0, direct_result.1,
    );
}

fn print_reference_points() {
    eprintln!();
    eprintln!("=== Reference Points ===");
    eprintln!("  D-Bus:        ~10,000 msg/sec (typical)");
    eprintln!("  gRPC (local): ~100,000-500,000 msg/sec");
    eprintln!("  Cap'n Proto:  ~500,000+ msg/sec");
    eprintln!("  io_uring:     ~1,000,000+ msg/sec");
}

// ── Test entry points ────────────────────────────────────────

/// Run all benchmarks sequentially for clean output.
/// Use: `cargo test --test bench_ipc --release -- --ignored --nocapture bench_all`
#[test]
#[ignore]
fn bench_all() {
    run_notification_throughput();
    run_request_reply_latency();
    run_fan_out();
    run_connection_source_write_paths();
    print_reference_points();
}

// Individual tests for running one benchmark at a time.

#[test]
#[ignore]
fn bench_notification_throughput() {
    run_notification_throughput();
}

#[test]
#[ignore]
fn bench_request_reply_latency() {
    run_request_reply_latency();
}

#[test]
#[ignore]
fn bench_fan_out() {
    run_fan_out();
}

#[test]
#[ignore]
fn bench_connection_source_direct_write() {
    run_connection_source_write_paths();
}

#[test]
#[ignore]
fn bench_reference_points() {
    print_reference_points();
}
