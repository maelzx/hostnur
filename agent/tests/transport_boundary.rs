//! Phase 11.2 socket/peer boundary checks.
//!
//! These prove that unauthorized peers, substituted sockets, malformed framing
//! and invalid requests all fail *before* request dispatch, using a real Unix
//! socket pair and kernel `SO_PEERCRED` credentials.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{FileTypeExt, MetadataExt, symlink};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use hostnur_agent::contract::{Catalog, MAX_MESSAGE_BYTES, OperationRequest};
use hostnur_agent::transport::{
    ConnectionOutcome, Listener, RequestHandler, ServerContext, TransportError, handle_connection,
};
use tempfile::TempDir;

/// Counts dispatches and returns a fixed JSON body.
struct RecordingHandler {
    calls: AtomicUsize,
    response: Option<serde_json::Value>,
}

impl RecordingHandler {
    fn responding() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            response: Some(serde_json::json!({"ok": true})),
        }
    }
}

impl RequestHandler for RecordingHandler {
    fn handle(&self, _request: &OperationRequest) -> Option<serde_json::Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.response.clone()
    }
}

fn current_uid() -> u32 {
    // The uid that owns a file we create is our effective uid.
    let dir = TempDir::new().unwrap();
    std::fs::metadata(dir.path()).unwrap().uid()
}

fn context<'a>(
    catalog: &'a Catalog,
    handler: &'a dyn RequestHandler,
    allowed: &'a [u32],
) -> ServerContext<'a> {
    ServerContext {
        catalog,
        handler,
        max_message_bytes: MAX_MESSAGE_BYTES,
        read_timeout: Duration::from_secs(2),
        allowed_peer_uids: allowed,
    }
}

const VALID_REQUEST: &str = r#"{
    "protocol_version": 1,
    "request_id": "6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7",
    "idempotency_key": "cf36a770-2abf-42e6-83cf-a0f042bcfc3c",
    "operation": "server.snapshot",
    "parameters": {}
}"#;

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

#[test]
fn binds_restrictive_socket_and_removes_on_drop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("agent.sock");

    let listener = Listener::bind(&path, 0o660).expect("bind");
    assert_eq!(listener.mode().unwrap(), 0o660);
    assert!(
        std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_socket()
    );

    drop(listener);
    assert!(!path.exists(), "socket must be removed on drop");
}

#[test]
fn refuses_symlink_at_socket_path() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.path().join("agent.sock");
    symlink(&target, &link).unwrap();

    let error = Listener::bind(&link, 0o660).expect_err("must refuse symlink");
    assert_eq!(error, TransportError::PathIsSymlink);
}

#[test]
fn refuses_non_socket_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("agent.sock");
    std::fs::write(&path, b"not a socket").unwrap();

    let error = Listener::bind(&path, 0o660).expect_err("must refuse");
    assert_eq!(error, TransportError::PathOccupied);
}

#[test]
fn rebinds_over_stale_socket() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("agent.sock");

    let listener = Listener::bind(&path, 0o660).unwrap();
    // Simulate an unclean exit that left the socket behind.
    std::mem::forget(listener);
    assert!(Path::new(&path).exists());

    let rebound = Listener::bind(&path, 0o660).expect("rebind over stale socket");
    assert!(rebound.mode().unwrap() & 0o777 == 0o660);
}

#[test]
fn allowed_peer_can_send_and_receive() {
    let catalog = Catalog::embedded();
    let handler = RecordingHandler::responding();
    let uid = current_uid();

    let (server, mut client) = UnixStream::pair().unwrap();
    client.write_all(&frame(VALID_REQUEST.as_bytes())).unwrap();

    let outcome = handle_connection(&server, &context(&catalog, &handler, &[uid]));
    assert!(
        matches!(outcome, ConnectionOutcome::Responded),
        "{outcome:?}"
    );
    assert_eq!(handler.calls.load(Ordering::SeqCst), 1);

    let mut response = vec![0u8; 4];
    client.read_exact(&mut response).unwrap();
    let length = u32::from_be_bytes(response.try_into().unwrap()) as usize;
    let mut body = vec![0u8; length];
    client.read_exact(&mut body).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["ok"], true);
}

#[test]
fn unauthorized_peer_is_rejected_before_dispatch() {
    let catalog = Catalog::embedded();
    let handler = RecordingHandler::responding();
    let uid = current_uid().wrapping_add(1);

    let (server, mut client) = UnixStream::pair().unwrap();
    // Even if the peer sends a valid request, an unlisted uid must be rejected.
    client.write_all(&frame(VALID_REQUEST.as_bytes())).unwrap();

    let outcome = handle_connection(&server, &context(&catalog, &handler, &[uid]));
    assert!(
        matches!(outcome, ConnectionOutcome::UnauthorizedPeer { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        handler.calls.load(Ordering::SeqCst),
        0,
        "dispatch must not run"
    );
}

#[test]
fn partial_frame_does_not_reach_handler() {
    let catalog = Catalog::embedded();
    let handler = RecordingHandler::responding();
    let uid = current_uid();

    let (server, mut client) = UnixStream::pair().unwrap();
    // Declare 10 bytes but send only 2, then half-close.
    client.write_all(&[0, 0, 0, 10, b'a', b'b']).unwrap();
    client.shutdown(Shutdown::Write).unwrap();

    let outcome = handle_connection(&server, &context(&catalog, &handler, &[uid]));
    assert!(
        matches!(outcome, ConnectionOutcome::Frame(_)),
        "{outcome:?}"
    );
    assert_eq!(handler.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn invalid_request_does_not_reach_handler() {
    let catalog = Catalog::embedded();
    let handler = RecordingHandler::responding();
    let uid = current_uid();

    let (server, mut client) = UnixStream::pair().unwrap();
    client.write_all(&frame(b"{}")).unwrap();

    let outcome = handle_connection(&server, &context(&catalog, &handler, &[uid]));
    assert!(
        matches!(outcome, ConnectionOutcome::InvalidRequest(_)),
        "{outcome:?}"
    );
    assert_eq!(handler.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn empty_allowlist_denies_everyone() {
    let catalog = Catalog::embedded();
    let handler = RecordingHandler::responding();

    let (server, _client) = UnixStream::pair().unwrap();
    let outcome = handle_connection(&server, &context(&catalog, &handler, &[]));
    assert!(
        matches!(outcome, ConnectionOutcome::UnauthorizedPeer { .. }),
        "{outcome:?}"
    );
    assert_eq!(handler.calls.load(Ordering::SeqCst), 0);
}
