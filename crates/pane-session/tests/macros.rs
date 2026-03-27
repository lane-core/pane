use pane_session::types::{Send, Recv, End, Chan};
use pane_session::{Offer, SessionError, choice, offer};
use pane_session::transport::memory;

/// 2-way choice using the macros.
type TwoWay = Send<String, Branch2>;
type Branch2 = pane_session::Branch<Recv<u64, End>, Recv<String, End>>;

#[test]
fn offer_macro_two_way_left() {
    let (client, server): (Chan<TwoWay, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        let server = server.select_left().unwrap();
        server.send(42u64).unwrap().close();
    });

    let client = client.send("test".to_string()).unwrap();
    let result: Result<u64, SessionError> = (|| {
        offer!(client, {
            accepted(c) => {
                let (val, c) = c.recv()?;
                c.close();
                Ok(val)
            },
            rejected(c) => {
                let (msg, c) = c.recv()?;
                c.close();
                Err(SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other, msg,
                )))
            },
        })
    })();

    assert_eq!(result.unwrap(), 42);
    server_handle.join().unwrap();
}

#[test]
fn offer_macro_two_way_right() {
    let (client, server): (Chan<TwoWay, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        let server = server.select_right().unwrap();
        server.send("nope".to_string()).unwrap().close();
    });

    let client = client.send("test".to_string()).unwrap();
    let result: Result<u64, SessionError> = (|| {
        offer!(client, {
            accepted(c) => {
                let (val, c) = c.recv()?;
                c.close();
                Ok(val)
            },
            rejected(c) => {
                let (msg, c) = c.recv()?;
                c.close();
                Err(SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other, msg,
                )))
            },
        })
    })();

    assert!(result.is_err());
    server_handle.join().unwrap();
}

/// 3-way choice using choice! for the SELECT (server) side.
/// The client sees the Branch (dual) side.
type AcceptPath = Send<u64, End>;       // server sends u64
type FallbackPath = Send<String, End>;  // server sends string
type RejectPath = End;                  // server just closes

// Server's choice type (what the server selects from)
type ThreeWaySelect = choice![AcceptPath, FallbackPath, RejectPath];
// Client protocol: send a string, then receive the server's choice
// The client sees the dual of ThreeWaySelect = Branch<Recv<u64,End>, Branch<Recv<String,End>, End>>
type ThreeWayProtocol = Send<String, pane_session::Dual<ThreeWaySelect>>;

// The server's dual of ThreeWayBranch is:
// Select<Send<u64, End>, Select<Send<String, End>, End>>

#[test]
fn three_way_choice_first_arm() {
    let (client, server): (Chan<ThreeWayProtocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        // Select first arm (accept)
        let server = server.select_left().unwrap();
        server.send(100u64).unwrap().close();
    });

    let client = client.send("go".to_string()).unwrap();
    let result: String = (|| -> Result<String, SessionError> {
        offer!(client, {
            accepted(c) => {
                let (val, c) = c.recv()?;
                c.close();
                Ok(format!("accepted: {}", val))
            },
            fallback(c) => {
                let (msg, c) = c.recv()?;
                c.close();
                Ok(format!("fallback: {}", msg))
            },
            rejected(c) => {
                c.close();
                Ok("rejected".to_string())
            },
        })
    })().unwrap();

    assert_eq!(result, "accepted: 100");
    server_handle.join().unwrap();
}

#[test]
fn three_way_choice_second_arm() {
    let (client, server): (Chan<ThreeWayProtocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        // Select second arm (fallback) — select_right then select_left
        let server = server.select_right().unwrap();
        let server = server.select_left().unwrap();
        server.send("try again".to_string()).unwrap().close();
    });

    let client = client.send("go".to_string()).unwrap();
    let result: String = (|| -> Result<String, SessionError> {
        offer!(client, {
            accepted(c) => {
                let (val, c) = c.recv()?;
                c.close();
                Ok(format!("accepted: {}", val))
            },
            fallback(c) => {
                let (msg, c) = c.recv()?;
                c.close();
                Ok(format!("fallback: {}", msg))
            },
            rejected(c) => {
                c.close();
                Ok("rejected".to_string())
            },
        })
    })().unwrap();

    assert_eq!(result, "fallback: try again");
    server_handle.join().unwrap();
}

#[test]
fn three_way_choice_third_arm() {
    let (client, server): (Chan<ThreeWayProtocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        // Select third arm (reject) — select_right then select_right
        let server = server.select_right().unwrap();
        let server = server.select_right().unwrap();
        server.close();
    });

    let client = client.send("go".to_string()).unwrap();
    let result: String = (|| -> Result<String, SessionError> {
        offer!(client, {
            accepted(c) => {
                let (val, c) = c.recv()?;
                c.close();
                Ok(format!("accepted: {}", val))
            },
            fallback(c) => {
                let (msg, c) = c.recv()?;
                c.close();
                Ok(format!("fallback: {}", msg))
            },
            rejected(c) => {
                c.close();
                Ok("rejected".to_string())
            },
        })
    })().unwrap();

    assert_eq!(result, "rejected");
    server_handle.join().unwrap();
}
