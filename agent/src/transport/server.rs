//! Connection pipeline: peer authorization, bounded framed read, parse, dispatch.
//!
//! One request/response per connection. The ordering matters: peer credentials
//! are checked *before* any message is read, framing is validated before JSON is
//! parsed, and a handler is only reached once the request has parsed. A failure
//! at any earlier stage must not reach dispatch.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::contract::{
    Catalog, ContractError, MAX_MESSAGE_BYTES, OperationRequest, encode_frame, parse_request,
    read_frame,
};
use crate::logging::{self, Level};

use super::peer::peer_credentials;
use super::{FrameError, Listener, TransportError};

/// Handles a parsed, authorized request. Returns the JSON response body, or
/// `None` when the connection should be closed without a body.
pub trait RequestHandler {
    fn handle(&self, request: &OperationRequest) -> Option<serde_json::Value>;
}

/// A handler that accepts and discards valid requests. Placeholder until the
/// real operation dispatch lands in a later slice.
pub struct NoopHandler;

impl RequestHandler for NoopHandler {
    fn handle(&self, _request: &OperationRequest) -> Option<serde_json::Value> {
        None
    }
}

/// Everything a connection needs, bundled to keep call sites and tests small.
pub struct ServerContext<'a> {
    pub catalog: &'a Catalog,
    pub handler: &'a dyn RequestHandler,
    pub max_message_bytes: usize,
    pub read_timeout: Duration,
    pub allowed_peer_uids: &'a [u32],
}

/// The observable result of handling one connection, used by `serve` logging and
/// by tests that assert dispatch was/was not reached.
#[derive(Debug)]
pub enum ConnectionOutcome {
    /// Peer UID was not in the allowlist; no message was read.
    UnauthorizedPeer { uid: u32 },
    /// Peer credentials could not be read.
    PeerCredentialsUnavailable,
    /// The peer closed cleanly before sending a message.
    Closed,
    /// Framing failed (zero/oversized/truncated) before JSON parsing.
    Frame(FrameError),
    /// The framed bytes were not a valid request.
    InvalidRequest(ContractError),
    /// Request parsed; handler produced and sent a response body.
    Responded,
    /// Request parsed; handler chose not to respond.
    HandledNoResponse,
    /// I/O failure while writing a response.
    Io(String),
}

/// Wall-clock bound for reading one complete request frame. The socket read
/// timeout applies per syscall, so a peer dripping one byte at a time could
/// otherwise hold the single-threaded serve loop open for the full declared
/// body size times the per-read timeout.
const FRAME_READ_DEADLINE: Duration = Duration::from_secs(30);

/// Reader that hands out per-syscall timeouts but enforces one overall
/// deadline for the whole frame.
struct DeadlineReader<'a> {
    stream: &'a UnixStream,
    deadline: Instant,
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.stream.read(buffer) {
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    if Instant::now() >= self.deadline {
                        return Err(err);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                result => return result,
            }
        }
    }
}

/// Handle a single accepted connection to completion.
pub fn handle_connection(stream: &UnixStream, context: &ServerContext<'_>) -> ConnectionOutcome {
    let peer = match peer_credentials(stream) {
        Ok(peer) => peer,
        Err(_) => return ConnectionOutcome::PeerCredentialsUnavailable,
    };

    if !peer.is_allowed(context.allowed_peer_uids) {
        return ConnectionOutcome::UnauthorizedPeer { uid: peer.uid };
    }

    // Without a socket timeout a stalled peer would block the single-threaded
    // loop indefinitely, so failure to install one closes the connection.
    if let Err(err) = stream.set_read_timeout(Some(context.read_timeout)) {
        return ConnectionOutcome::Io(format!("setting read timeout failed: {err}"));
    }
    if let Err(err) = stream.set_write_timeout(Some(context.read_timeout)) {
        return ConnectionOutcome::Io(format!("setting write timeout failed: {err}"));
    }

    let mut reader = DeadlineReader {
        stream,
        deadline: Instant::now() + FRAME_READ_DEADLINE,
    };
    let bytes = match read_frame(&mut reader, context.max_message_bytes) {
        Ok(bytes) => bytes,
        Err(FrameError::ConnectionClosed) => return ConnectionOutcome::Closed,
        Err(error) => return ConnectionOutcome::Frame(error),
    };

    let request = match parse_request(&bytes, context.catalog) {
        Ok(request) => request,
        Err(error) => {
            // Return the stable machine error code as a framed response (the
            // contract's error object) rather than closing silently.
            let body = contract_error_body(&error);
            let _ = send_response(stream, &body, context.max_message_bytes);
            return ConnectionOutcome::InvalidRequest(error);
        }
    };

    match context.handler.handle(&request) {
        None => ConnectionOutcome::HandledNoResponse,
        Some(body) => match send_response(stream, &body, context.max_message_bytes) {
            Ok(()) => ConnectionOutcome::Responded,
            Err(message) => ConnectionOutcome::Io(message),
        },
    }
}

/// The contract's error object for a request-level failure.
fn contract_error_body(error: &ContractError) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "code": error.code().as_str(),
            "message": error.message(),
            "retryable": false,
        }
    })
}

fn send_response(
    stream: &UnixStream,
    body: &serde_json::Value,
    max_message_bytes: usize,
) -> Result<(), String> {
    let encoded = serde_json::to_vec(body).map_err(|err| err.to_string())?;
    let frame = encode_frame(&encoded, max_message_bytes).map_err(|err| err.to_string())?;
    let mut writer = stream;
    writer.write_all(&frame).map_err(|err| err.to_string())?;
    writer.flush().map_err(|err| err.to_string())
}

/// Accept and handle connections until `shutdown` is set.
///
/// Acceptance is non-blocking so the shutdown flag is observed promptly;
/// connections are handled one at a time, which bounds concurrency to one
/// in-flight request for this initial transport.
pub fn serve(
    listener: &Listener,
    context: &ServerContext<'_>,
    shutdown: &AtomicBool,
) -> Result<(), TransportError> {
    listener
        .set_nonblocking(true)
        .map_err(|err| TransportError::Bind(format!("setting listener non-blocking: {err}")))?;

    // The listener is only created after the parent directory exists.
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let outcome = handle_connection(&stream, context);
                log_outcome(&outcome);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                logging::log(
                    Level::Warn,
                    "transport",
                    "accept.failed",
                    &format!("accept failed: {}", err.kind()),
                    Some("transport"),
                    None,
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    Ok(())
}

fn log_outcome(outcome: &ConnectionOutcome) {
    match outcome {
        ConnectionOutcome::UnauthorizedPeer { uid } => logging::log(
            Level::Warn,
            "transport",
            "peer.rejected",
            &format!("rejected connection from uid {uid}"),
            Some("permission_denied"),
            None,
        ),
        ConnectionOutcome::PeerCredentialsUnavailable => logging::log(
            Level::Warn,
            "transport",
            "peer.credentials_unavailable",
            "could not read peer credentials",
            Some("permission_denied"),
            None,
        ),
        ConnectionOutcome::Frame(error) => logging::log(
            Level::Warn,
            "transport",
            "frame.rejected",
            &format!("rejected frame: {error}"),
            Some("invalid_parameters"),
            None,
        ),
        ConnectionOutcome::InvalidRequest(error) => logging::log(
            Level::Warn,
            "transport",
            "request.rejected",
            &format!("rejected request: {}", error.message()),
            Some(error.code().as_str()),
            None,
        ),
        ConnectionOutcome::Io(message) => logging::log(
            Level::Warn,
            "transport",
            "response.failed",
            &format!("writing response failed: {message}"),
            Some("io"),
            None,
        ),
        ConnectionOutcome::Closed
        | ConnectionOutcome::Responded
        | ConnectionOutcome::HandledNoResponse => {}
    }
}

/// Default maximum message size for the transport when a config value is absent.
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = MAX_MESSAGE_BYTES;
