// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Lower-case hex formatting for digests.
//!
//! RustCrypto digests (sha2 0.11 and later) no longer implement `LowerHex`, so
//! `format!("{:x}", hasher.finalize())` stopped compiling; use this instead.

/// Lower-case hexadecimal encoding of `bytes`.
pub fn hex_lower(bytes: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_lowercase() {
        assert_eq!(hex_lower([0x00, 0xab, 0xFF]), "00abff");
        assert_eq!(hex_lower([]), "");
    }
}
