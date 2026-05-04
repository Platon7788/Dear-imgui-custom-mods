//! Allocation-free byte → hex string conversion.
//!
//! Both `hex_viewer` and `disasm_view` repeatedly format individual bytes
//! as `"{:02X}"` / `"{:02x}"` during their inner per-row drawing loops. A
//! 16-byte/row hex view with 60+ visible rows formats > 1 000 bytes per
//! frame; running that through `format!()` allocates a fresh `String` on
//! the heap each time. This module replaces those calls with an O(1)
//! `&'static str` lookup against a 256-entry table built once on first use.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::utils::hex::{byte_hex_upper, byte_hex_lower};
//!
//! draw_list.add_text([x, y], col, byte_hex_upper(b));   // "FF"
//! draw_list.add_text([x, y], col, byte_hex_lower(b));   // "ff"
//! ```
//!
//! Both helpers return `&'static str` of length 2 — never allocate, never
//! panic. The table is initialised on first call via `OnceLock` and lives
//! for the life of the process.

use std::sync::OnceLock;

/// `&'static` hex pair for a byte using uppercase `A..F`.
#[inline]
pub fn byte_hex_upper(b: u8) -> &'static str {
    static TABLE: OnceLock<[Box<str>; 256]> = OnceLock::new();
    let t = TABLE.get_or_init(|| std::array::from_fn(|i| format!("{i:02X}").into_boxed_str()));
    &t[b as usize]
}

/// `&'static` hex pair for a byte using lowercase `a..f`.
#[inline]
pub fn byte_hex_lower(b: u8) -> &'static str {
    static TABLE: OnceLock<[Box<str>; 256]> = OnceLock::new();
    let t = TABLE.get_or_init(|| std::array::from_fn(|i| format!("{i:02x}").into_boxed_str()));
    &t[b as usize]
}

/// Convenience: pick upper / lower at runtime.
#[inline]
pub fn byte_hex(b: u8, upper: bool) -> &'static str {
    if upper {
        byte_hex_upper(b)
    } else {
        byte_hex_lower(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_round_trip() {
        for b in 0u16..=255 {
            let b = b as u8;
            assert_eq!(byte_hex_upper(b), format!("{b:02X}"));
        }
    }

    #[test]
    fn lower_round_trip() {
        for b in 0u16..=255 {
            let b = b as u8;
            assert_eq!(byte_hex_lower(b), format!("{b:02x}"));
        }
    }

    #[test]
    fn returns_static_str() {
        // Calling twice returns the same pointer — table is cached.
        let a = byte_hex_upper(0xAB);
        let b = byte_hex_upper(0xAB);
        assert_eq!(a.as_ptr(), b.as_ptr());
    }

    #[test]
    fn dispatcher_picks_case() {
        assert_eq!(byte_hex(0xCD, true), "CD");
        assert_eq!(byte_hex(0xCD, false), "cd");
    }
}
