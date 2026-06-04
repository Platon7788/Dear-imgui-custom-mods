//! JSON formatter + minimal parser for [`FlatExportData`] / [`TreeExportNode`].

use super::{FieldValue, FlatExportData, TreeExportNode};

// ── Formatter ────────────────────────────────────────────────────────────────

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Reverse of [`json_escape`]: decode the escape sequences `\"` `\\` `\/`
/// `\n` `\r` `\t` `\b` `\f` `\uXXXX` in a single left-to-right pass.
///
/// A single pass is mandatory — the previous implementation chained
/// `String::replace` calls (`\\"` → `\n` → `\t` → `\\`), which corrupted
/// any string containing a literal backslash followed by `n`/`t` (the
/// `\n` replace fired inside the *doubled* backslash sequence) and never
/// decoded `\r` or `\uXXXX` at all.
fn json_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    // Malformed \u escape — keep it verbatim rather than
                    // dropping data.
                    None => {
                        out.push('\\');
                        out.push('u');
                        out.push_str(&hex);
                    }
                }
            }
            // Unknown escape — preserve the backslash and the next char.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            // Trailing lone backslash.
            None => out.push('\\'),
        }
    }
    out
}

fn field_value_json(v: &FieldValue) -> String {
    match v {
        FieldValue::Null => "null".into(),
        FieldValue::Bool(b) => b.to_string(),
        FieldValue::Int(i) => i.to_string(),
        FieldValue::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                "null".into()
            } else {
                format!("{}", f)
            }
        }
        FieldValue::Str(s) => format!("\"{}\"", json_escape(s)),
        FieldValue::Color(c) => format!("[{:.3}, {:.3}, {:.3}, {:.3}]", c[0], c[1], c[2], c[3]),
    }
}

pub(super) fn format_flat(data: &FlatExportData) -> String {
    let mut out = String::from("[\n");
    for (ri, row) in data.rows.iter().enumerate() {
        out.push_str("  {");
        for (ci, val) in row.iter().enumerate() {
            if ci > 0 {
                out.push_str(", ");
            }
            let key = data.columns.get(ci).map(|s| s.as_str()).unwrap_or("?");
            out.push_str(&format!(
                "\"{}\": {}",
                json_escape(key),
                field_value_json(val)
            ));
        }
        out.push('}');
        if ri + 1 < data.rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

pub(super) fn format_tree(nodes: &[TreeExportNode], indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let mut out = String::from("[\n");
    for (i, node) in nodes.iter().enumerate() {
        out.push_str(&format!("{}  {{\n", pad));
        for (fi, (key, val)) in node.fields.iter().enumerate() {
            out.push_str(&format!(
                "{}  \"{}\": {}",
                pad1,
                json_escape(key),
                field_value_json(val)
            ));
            if fi + 1 < node.fields.len() || !node.children.is_empty() {
                out.push(',');
            }
            out.push('\n');
        }
        if !node.children.is_empty() {
            out.push_str(&format!(
                "{}  \"children\": {}\n",
                pad1,
                format_tree(&node.children, indent + 2)
            ));
        }
        out.push_str(&format!("{}  }}", pad));
        if i + 1 < nodes.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", pad));
    out
}

// ── Parser (minimal) ─────────────────────────────────────────────────────────

pub(super) fn parse_flat(content: &str) -> Option<FlatExportData> {
    // Minimal JSON array-of-objects parser.
    let content = content.trim();
    if !content.starts_with('[') || !content.ends_with(']') {
        return None;
    }
    let inner = &content[1..content.len() - 1];

    let mut columns = Vec::new();
    let mut rows = Vec::new();
    let mut col_set = std::collections::HashSet::new();

    // Split objects (simplified — doesn't handle nested objects/arrays).
    for obj_str in split_json_objects(inner) {
        let fields = parse_json_object(obj_str.trim());
        // Collect column names from first row.
        if rows.is_empty() {
            for (key, _) in &fields {
                if col_set.insert(key.clone()) {
                    columns.push(key.clone());
                }
            }
        }
        // Build row values aligned to columns.
        let mut row = vec![FieldValue::Null; columns.len()];
        for (key, val) in &fields {
            if let Some(idx) = columns.iter().position(|c| c == key) {
                row[idx] = val.clone();
            }
        }
        rows.push(row);
    }

    Some(FlatExportData { columns, rows })
}

fn split_json_objects(s: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    let bytes = s.as_bytes();
    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' && in_string {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        if b == b'{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                if let Some(s_idx) = start {
                    results.push(&s[s_idx..=i]);
                }
                start = None;
            }
        }
    }
    results
}

fn parse_json_object(s: &str) -> Vec<(String, FieldValue)> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return Vec::new();
    }
    let inner = s[1..s.len() - 1].trim();
    if inner.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    let mut remaining = inner;

    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches([',', ' ', '\n', '\r', '\t']);
        if remaining.is_empty() {
            break;
        }

        // Parse key.
        if !remaining.starts_with('"') {
            break;
        }
        let key_end = remaining[1..].find('"').map(|p| p + 1);
        let Some(ke) = key_end else { break };
        let key = remaining[1..ke].to_string();
        remaining = &remaining[ke + 1..];

        // Skip colon.
        remaining = remaining.trim_start();
        if remaining.starts_with(':') {
            remaining = &remaining[1..];
        }
        remaining = remaining.trim_start();

        // Parse value.
        let (val, rest) = parse_json_value(remaining);
        fields.push((key, val));
        remaining = rest.trim_start_matches([',', ' ', '\n', '\r', '\t']);
    }

    fields
}

fn parse_json_value(s: &str) -> (FieldValue, &str) {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('"') {
        // String value. `close` is the byte index of the terminating
        // quote, or `None` when the string is unterminated (malformed
        // input) — in which case we consume the remainder rather than
        // slicing at byte 1, which could split a multi-byte char and
        // panic.
        let mut close: Option<usize> = None;
        let mut escape = false;
        for (i, b) in rest.bytes().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            if b == b'\\' {
                escape = true;
                continue;
            }
            if b == b'"' {
                close = Some(i);
                break;
            }
        }
        match close {
            Some(end) => (
                FieldValue::Str(json_unescape(&rest[..end])),
                &rest[end + 1..],
            ),
            None => (FieldValue::Str(json_unescape(rest)), ""),
        }
    } else if let Some(rest) = s.strip_prefix("null") {
        (FieldValue::Null, rest)
    } else if let Some(rest) = s.strip_prefix("true") {
        (FieldValue::Bool(true), rest)
    } else if let Some(rest) = s.strip_prefix("false") {
        (FieldValue::Bool(false), rest)
    } else if s.starts_with('[') {
        // Skip arrays (colors, children).
        let mut depth = 0i32;
        // Default to the whole slice when the array is unterminated, so a
        // missing `]` consumes the rest instead of underflowing `end - 1`
        // (which panicked on malformed input before this guard).
        let mut end = s.len();
        for (i, b) in s.bytes().enumerate() {
            if b == b'[' {
                depth += 1;
            }
            if b == b']' {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
        }
        // Try parse as color [f32; 4]. `end >= 1` here (we matched `[`),
        // so `end - 1 >= 0` and the slice is always in bounds.
        let arr_str = &s[1..end - 1];
        let nums: Vec<f32> = arr_str
            .split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            (
                FieldValue::Color([nums[0], nums[1], nums[2], nums[3]]),
                &s[end..],
            )
        } else {
            (FieldValue::Str(s[..end].to_string()), &s[end..])
        }
    } else {
        // Number.
        let end = s.find([',', '}', ']', '\n', ' ']).unwrap_or(s.len());
        let num_str = s[..end].trim();
        if num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
            let f = num_str.parse::<f64>().unwrap_or(0.0);
            (FieldValue::Float(f), &s[end..])
        } else {
            let i = num_str.parse::<i64>().unwrap_or(0);
            (FieldValue::Int(i), &s[end..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_row(s: &str) -> FlatExportData {
        let mut d = FlatExportData::new(vec!["v".into()]);
        d.add_row(vec![FieldValue::Str(s.into())]);
        d
    }

    fn round_trip_str(s: &str) -> String {
        let data = one_row(s);
        let json = format_flat(&data);
        let parsed = parse_flat(&json).expect("parse");
        match &parsed.rows[0][0] {
            FieldValue::Str(out) => out.clone(),
            other => panic!("expected Str, got {other:?}"),
        }
    }

    #[test]
    fn unescape_backslash_then_letter_n_not_corrupted() {
        // Regression: chained `.replace("\\n", ...)` used to fire inside the
        // doubled-backslash escape, turning `\` + 'n' into `\` + newline.
        assert_eq!(round_trip_str("a\\nb"), "a\\nb");
        assert_eq!(round_trip_str("a\\tb"), "a\\tb");
        assert_eq!(round_trip_str("path\\to\\nowhere"), "path\\to\\nowhere");
    }

    #[test]
    fn unescape_real_control_chars_round_trip() {
        assert_eq!(round_trip_str("new\nline"), "new\nline");
        assert_eq!(round_trip_str("tab\there"), "tab\there");
        // Regression: `\r` was escaped on write but never decoded on read.
        assert_eq!(round_trip_str("cr\rret"), "cr\rret");
        assert_eq!(round_trip_str("quote\"q"), "quote\"q");
        assert_eq!(round_trip_str("bs\\bs"), "bs\\bs");
    }

    #[test]
    fn unescape_decodes_u_escape() {
        // `` control char is emitted via `\uXXXX` and must decode.
        assert_eq!(round_trip_str("ctrl\u{0001}x"), "ctrl\u{0001}x");
        assert_eq!(json_unescape("\\u0041"), "A");
    }

    #[test]
    fn unescape_keeps_unknown_escape_verbatim() {
        assert_eq!(json_unescape("\\q"), "\\q");
        assert_eq!(json_unescape("trailing\\"), "trailing\\");
        // Malformed \u (short / non-hex) preserved rather than dropped.
        assert_eq!(json_unescape("\\uZZZZ"), "\\uZZZZ");
    }

    #[test]
    fn parse_unterminated_array_does_not_panic() {
        // Regression: missing `]` left `end == 0`, and `&s[1..end - 1]`
        // underflowed `usize` and panicked.
        let (val, _rest) = parse_json_value("[1, 2, 3");
        // Three numbers != 4, so falls back to a string — the point is no panic.
        assert!(matches!(val, FieldValue::Str(_)));
    }

    #[test]
    fn parse_unterminated_string_does_not_panic() {
        // Multi-byte first char + no closing quote previously risked slicing
        // mid-codepoint. Must consume the rest instead.
        let (val, rest) = parse_json_value("\"\u{00e9}abc");
        assert_eq!(rest, "");
        match val {
            FieldValue::Str(s) => assert_eq!(s, "\u{00e9}abc"),
            other => panic!("expected Str, got {other:?}"),
        }
    }

    #[test]
    fn color_round_trips_through_array() {
        let mut d = FlatExportData::new(vec!["c".into()]);
        d.add_row(vec![FieldValue::Color([0.1, 0.2, 0.3, 1.0])]);
        let json = format_flat(&d);
        let parsed = parse_flat(&json).unwrap();
        match &parsed.rows[0][0] {
            FieldValue::Color(c) => {
                assert!((c[0] - 0.1).abs() < 1e-3);
                assert!((c[3] - 1.0).abs() < 1e-3);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn parse_empty_array_yields_no_rows() {
        let parsed = parse_flat("[]").unwrap();
        assert!(parsed.rows.is_empty());
    }

    #[test]
    fn nan_and_infinite_floats_serialize_as_null() {
        let mut d = FlatExportData::new(vec!["f".into()]);
        d.add_row(vec![FieldValue::Float(f64::NAN)]);
        d.add_row(vec![FieldValue::Float(f64::INFINITY)]);
        let json = format_flat(&d);
        assert_eq!(json.matches("null").count(), 2);
    }
}
