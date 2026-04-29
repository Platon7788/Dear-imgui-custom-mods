//! Selection model + wildcard byte search + clipboard-format conversion.
//!
//! Pure pattern/data utilities live as `pub(super) fn` free functions
//! (testable in isolation; no `&self` overhead). The single
//! `HexViewer::do_search` impl is here too because it is the only
//! method that mutates `search_*` state.

use super::HexViewer;
use super::config::{CopyFormat, HexSearchMode};

// ── Selection ────────────────────────────────────────────────────────────────

/// Byte range selection. The interval is half-open: `[start, end)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
    pub fn contains(&self, offset: usize) -> bool {
        let (lo, hi) = self.ordered();
        offset >= lo && offset < hi
    }
    pub fn ordered(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
    pub fn len(&self) -> usize {
        let (lo, hi) = self.ordered();
        hi - lo
    }
}

// ── Wildcard pattern ─────────────────────────────────────────────────────────

/// A single byte in a search pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternByte {
    Exact(u8),
    Any,
}

/// Parse a whitespace-delimited hex pattern with `??` / `?` wildcards
/// (e.g. `"4D 5A ?? 00"`). Tokens that are not valid hex bytes nor
/// wildcards are silently skipped — convenient for partial input.
pub(super) fn parse_hex_pattern_masked(s: &str) -> Vec<PatternByte> {
    s.split_whitespace()
        .filter_map(|tok| {
            if tok == "??" || tok == "?" {
                Some(PatternByte::Any)
            } else {
                u8::from_str_radix(tok, 16).ok().map(PatternByte::Exact)
            }
        })
        .collect()
}

/// Treat the input as raw bytes (UTF-8 byte-level, **not** char-level
/// — matches on wire bytes regardless of multibyte boundaries).
pub(super) fn parse_ascii_pattern(s: &str) -> Vec<PatternByte> {
    s.bytes().map(PatternByte::Exact).collect()
}

/// Naive O(N·M) scan with wildcard support. Sufficient for interactive
/// search of buffers up to a few MiB; switch to Boyer-Moore-like
/// preprocessing if we ever need to search GB-scale buffers.
///
/// The result vec is **sorted ascending** by construction (we walk
/// `data` left-to-right) — `is_search_match` relies on that for its
/// binary-search probe.
pub(super) fn find_pattern_masked(data: &[u8], pattern: &[PatternByte]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > data.len() {
        return Vec::new();
    }
    let mut results = Vec::new();
    'outer: for i in 0..=data.len() - pattern.len() {
        for (j, pb) in pattern.iter().enumerate() {
            match pb {
                PatternByte::Any => {}
                PatternByte::Exact(b) => {
                    if data[i + j] != *b {
                        continue 'outer;
                    }
                }
            }
        }
        results.push(i);
    }
    results
}

// ── Clipboard format conversion ──────────────────────────────────────────────

pub(super) fn format_bytes(bytes: &[u8], format: CopyFormat, uppercase: bool) -> String {
    match format {
        CopyFormat::HexSpaced => bytes
            .iter()
            .map(|b| {
                if uppercase {
                    format!("{:02X}", b)
                } else {
                    format!("{:02x}", b)
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        CopyFormat::HexCompact => bytes
            .iter()
            .map(|b| {
                if uppercase {
                    format!("{:02X}", b)
                } else {
                    format!("{:02x}", b)
                }
            })
            .collect::<String>(),
        CopyFormat::CArray => {
            let inner: String = bytes
                .iter()
                .map(|b| {
                    if uppercase {
                        format!("0x{:02X}", b)
                    } else {
                        format!("0x{:02x}", b)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {} }}", inner)
        }
        CopyFormat::RustArray => {
            let inner: String = bytes
                .iter()
                .map(|b| {
                    if uppercase {
                        format!("0x{:02X}", b)
                    } else {
                        format!("0x{:02x}", b)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        CopyFormat::Base64 => base64_encode(bytes),
        CopyFormat::Ascii => bytes
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect(),
    }
}

/// In-house Base64 (RFC 4648) — avoids pulling a dependency for a
/// 20-line routine. Pre-allocates the exact output capacity.
pub(super) fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ── Address parsing ──────────────────────────────────────────────────────────

/// Parse an address typed in the goto popup.
///
/// Supports `0x`-prefixed hex, all-hex-digit strings longer than 4 chars
/// (treated as hex even without `0x` — the goto field is much more often
/// hex than decimal in practice), and bare decimal otherwise. `≤4` char
/// all-hex-digit strings are decimal so `100` doesn't surprise users.
pub(super) fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() > 4 {
        u64::from_str_radix(s, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

// ── Search execution ────────────────────────────────────────────────────────

impl HexViewer {
    pub(super) fn do_search(&mut self) {
        self.search_pattern = match self.config.search_mode {
            HexSearchMode::Hex => parse_hex_pattern_masked(&self.search_buf),
            HexSearchMode::Ascii => parse_ascii_pattern(&self.search_buf),
        };
        if self.search_pattern.is_empty() {
            return;
        }

        self.search_results = find_pattern_masked(&self.data, &self.search_pattern);
        self.search_idx = 0;

        if !self.search_results.is_empty() {
            self.cursor = self.search_results[0];
            self.selection = Selection {
                start: self.cursor,
                end: self.cursor + self.search_pattern.len(),
            };
            self.scroll_to_cursor();
        }
    }
}
