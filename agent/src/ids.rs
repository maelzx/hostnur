//! Identifier generation for agent-owned records.

use crate::contract::Uuid;

/// Generate a random version-4 UUID using the operating system CSPRNG.
pub fn new_uuid_v4() -> Uuid {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("operating system RNG must be available");
    // Set version (4) and variant (RFC 4122) bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut hex = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            hex.push('-');
        }
        hex.push_str(&format!("{byte:02x}"));
    }

    Uuid::parse(&hex).expect("generated UUID is canonical")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_canonical_v4_uuids() {
        let id = new_uuid_v4();
        assert_eq!(id.as_str().len(), 36);
        assert_eq!(&id.as_str()[14..15], "4");
        assert!(matches!(&id.as_str()[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn generated_ids_are_unique() {
        let a = new_uuid_v4();
        let b = new_uuid_v4();
        assert_ne!(a, b);
    }
}
