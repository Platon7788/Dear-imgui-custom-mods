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
        // String value.
        let mut end = 0;
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
                end = i;
                break;
            }
        }
        let val = rest[..end]
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\\", "\\");
        (FieldValue::Str(val), &rest[end + 1..])
    } else if let Some(rest) = s.strip_prefix("null") {
        (FieldValue::Null, rest)
    } else if let Some(rest) = s.strip_prefix("true") {
        (FieldValue::Bool(true), rest)
    } else if let Some(rest) = s.strip_prefix("false") {
        (FieldValue::Bool(false), rest)
    } else if s.starts_with('[') {
        // Skip arrays (colors, children).
        let mut depth = 0i32;
        let mut end = 0;
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
        // Try parse as color [f32; 4].
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
