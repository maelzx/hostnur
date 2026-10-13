//! Length-prefixed message framing for the local transport.
//!
//! Each message is a four-byte unsigned big-endian length followed by exactly
//! that many bytes of UTF-8 JSON. The length is validated against a hard cap
//! *before* any allocation for the body, so a hostile or corrupt peer cannot
//! force unbounded memory use. One request/response per connection is assumed
//! for now; correlation is carried by `request_id` in the envelope.

use std::io::{self, Read};

/// Hard cap for a single protocol message (1 MiB), matching
/// `contracts/v1/README.md` and the panel's `hostnur.limits.max_message_bytes`.
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// Framing failures that occur before JSON parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The peer closed the connection before a full length prefix was read.
    ConnectionClosed,
    /// A zero-length message is never a valid frame.
    ZeroLength,
    /// The declared length exceeds the negotiated/global cap.
    TooLarge { declared: u32, max: usize },
    /// The stream ended part-way through a message body.
    Truncated { expected: usize, read: usize },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::ConnectionClosed => f.write_str("connection closed before a full frame"),
            FrameError::ZeroLength => f.write_str("zero-length frame"),
            FrameError::TooLarge { declared, max } => {
                write!(f, "declared length {declared} exceeds maximum {max}")
            }
            FrameError::Truncated { expected, read } => {
                write!(f, "truncated frame: expected {expected} bytes, read {read}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Encode a payload as a length-prefixed frame.
///
/// Returns an error message if the payload exceeds `max`.
pub fn encode_frame(payload: &[u8], max: usize) -> Result<Vec<u8>, FrameError> {
    if payload.len() > max || payload.len() > u32::MAX as usize {
        return Err(FrameError::TooLarge {
            declared: payload.len().min(u32::MAX as usize) as u32,
            max,
        });
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Read exactly one length-prefixed frame from `reader`.
///
/// `max` caps the accepted body size. A clean EOF before the length prefix is
/// reported as `ConnectionClosed`; an EOF part-way through a body is
/// `Truncated` and must never be treated as an accepted message.
pub fn read_frame<R: Read>(reader: &mut R, max: usize) -> Result<Vec<u8>, FrameError> {
    let mut prefix = [0u8; 4];
    read_exact_or(reader, &mut prefix, true)?;

    let declared = u32::from_be_bytes(prefix);
    if declared == 0 {
        return Err(FrameError::ZeroLength);
    }
    if declared as usize > max {
        return Err(FrameError::TooLarge { declared, max });
    }

    let mut body = vec![0u8; declared as usize];
    read_exact_or(reader, &mut body, false)?;
    Ok(body)
}

/// Fill `buffer`, distinguishing a clean initial EOF from a mid-stream one.
fn read_exact_or<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    clean_eof_allowed: bool,
) -> Result<(), FrameError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => {
                if filled == 0 && clean_eof_allowed {
                    return Err(FrameError::ConnectionClosed);
                }
                return Err(FrameError::Truncated {
                    expected: buffer.len(),
                    read: filled,
                });
            }
            Ok(n) => filled += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err(FrameError::Truncated {
                    expected: buffer.len(),
                    read: filled,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trips_a_payload() {
        let frame = encode_frame(b"{\"ok\":true}", MAX_MESSAGE_BYTES).unwrap();
        assert_eq!(&frame[..4], &[0, 0, 0, 11]);
        let mut cursor = Cursor::new(frame);
        let body = read_frame(&mut cursor, MAX_MESSAGE_BYTES).unwrap();
        assert_eq!(body, b"{\"ok\":true}");
    }

    #[test]
    fn rejects_zero_length() {
        let mut cursor = Cursor::new(vec![0u8, 0, 0, 0]);
        assert_eq!(
            read_frame(&mut cursor, MAX_MESSAGE_BYTES).unwrap_err(),
            FrameError::ZeroLength
        );
    }

    #[test]
    fn rejects_oversized_declaration_before_allocating() {
        let mut cursor = Cursor::new(vec![0xff, 0xff, 0xff, 0xff]);
        assert_eq!(
            read_frame(&mut cursor, 1024).unwrap_err(),
            FrameError::TooLarge {
                declared: u32::MAX,
                max: 1024,
            }
        );
    }

    #[test]
    fn rejects_truncated_body() {
        let mut cursor = Cursor::new(vec![0u8, 0, 0, 10, b'a', b'b']);
        assert_eq!(
            read_frame(&mut cursor, MAX_MESSAGE_BYTES).unwrap_err(),
            FrameError::Truncated {
                expected: 10,
                read: 2,
            }
        );
    }

    #[test]
    fn rejects_empty_stream() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        assert_eq!(
            read_frame(&mut cursor, MAX_MESSAGE_BYTES).unwrap_err(),
            FrameError::ConnectionClosed
        );
    }

    #[test]
    fn encode_rejects_payload_over_cap() {
        let payload = vec![0u8; 32];
        assert!(encode_frame(&payload, 16).is_err());
    }
}
