//! Export/import system for VirtualTable and VirtualTree data.
//!
//! Provides format-agnostic serialization traits and built-in formatters
//! for JSON, YAML, RON, and TXT. No external dependencies — pure Rust.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::utils::export::*;
//!
//! // Implement Exportable for your row/node type:
//! struct MyRow { name: String, value: f64 }
//!
//! impl Exportable for MyRow {
//!     fn field_names() -> &'static [&'static str] { &["name", "value"] }
//!     fn field_value(&self, col: usize) -> FieldValue {
//!         match col {
//!             0 => FieldValue::Str(self.name.clone()),
//!             1 => FieldValue::Float(self.value),
//!             _ => FieldValue::Null,
//!         }
//!     }
//! }
//! ```

use std::path::Path;

// ── Export Format ────────────────────────────────────────────────────────────

/// Supported export/import formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Yaml,
    Ron,
    Txt,
}

impl ExportFormat {
    /// All supported formats.
    pub const ALL: &'static [ExportFormat] = &[
        Self::Json, Self::Yaml, Self::Ron, Self::Txt,
    ];

    /// File extension (without dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Ron  => "ron",
            Self::Txt  => "txt",
        }
    }

    /// Display name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Ron  => "RON",
            Self::Txt  => "Text",
        }
    }

    /// Detect format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "ron" => Some(Self::Ron),
            "txt" | "text" | "tsv" | "csv" => Some(Self::Txt),
            _ => None,
        }
    }

    /// Detect format from file path.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }
}

// ── Field Value ─────────────────────────────────────────────────────────────

/// A typed field value for serialization.
#[derive(Debug, Clone)]
pub enum FieldValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// RGBA color as [f32; 4].
    Color([f32; 4]),
}

impl FieldValue {
    /// Convert to display string.
    pub fn to_string_lossy(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => format!("{}", f),
            Self::Str(s) => s.clone(),
            Self::Color(c) => format!("[{:.3}, {:.3}, {:.3}, {:.3}]", c[0], c[1], c[2], c[3]),
        }
    }
}

// ── Exportable Trait ────────────────────────────────────────────────────────

/// Trait for types that can be exported to structured formats.
///
/// Implement on your row/node data type to enable export.
pub trait Exportable {
    /// Column/field names for the header row.
    fn field_names() -> &'static [&'static str];

    /// Get the value of field at `col` index.
    fn field_value(&self, col: usize) -> FieldValue;

    /// Number of fields.
    fn field_count() -> usize { Self::field_names().len() }
}

// ── Importable Trait ────────────────────────────────────────────────────────

/// Trait for types that can be imported (deserialized) from structured formats.
///
/// Implement on your row/node data type to enable import.
pub trait Importable: Sized {
    /// Create an instance from a map of field_name → FieldValue.
    fn from_fields(fields: &[(&str, FieldValue)]) -> Option<Self>;
}

// ── Tree Export Node ────────────────────────────────────────────────────────

/// Represents a tree node with its children for hierarchical export.
#[derive(Debug, Clone)]
pub struct TreeExportNode {
    /// Field values for this node.
    pub fields: Vec<(String, FieldValue)>,
    /// Child nodes (recursive).
    pub children: Vec<TreeExportNode>,
}

// ── Flat Row Export ─────────────────────────────────────────────────────────

/// Holds a collection of flat rows ready for export.
pub struct FlatExportData {
    /// Column names.
    pub columns: Vec<String>,
    /// Rows: each row is a Vec of FieldValues matching columns.
    pub rows: Vec<Vec<FieldValue>>,
}

impl FlatExportData {
    pub fn new(columns: Vec<String>) -> Self {
        Self { columns, rows: Vec::new() }
    }

    pub fn add_row(&mut self, row: Vec<FieldValue>) {
        self.rows.push(row);
    }
}

// ── Formatters ──────────────────────────────────────────────────────────────

/// Format flat table data to string.
pub fn format_flat(data: &FlatExportData, format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => format_flat_json(data),
        ExportFormat::Yaml => format_flat_yaml(data),
        ExportFormat::Ron  => format_flat_ron(data),
        ExportFormat::Txt  => format_flat_txt(data),
    }
}

/// Format hierarchical tree data to string.
pub fn format_tree(nodes: &[TreeExportNode], format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => format_tree_json(nodes, 0),
        ExportFormat::Yaml => format_tree_yaml(nodes, 0),
        ExportFormat::Ron  => format_tree_ron(nodes, 0),
        ExportFormat::Txt  => format_tree_txt(nodes, 0),
    }
}

/// Export flat data to file.
pub fn export_flat_to_file(
    data: &FlatExportData,
    path: &Path,
    format: Option<ExportFormat>,
) -> std::io::Result<()> {
    let fmt = format.or_else(|| ExportFormat::from_path(path))
        .unwrap_or(ExportFormat::Json);
    let content = format_flat(data, fmt);
    std::fs::write(path, content)
}

/// Export tree data to file.
pub fn export_tree_to_file(
    nodes: &[TreeExportNode],
    path: &Path,
    format: Option<ExportFormat>,
) -> std::io::Result<()> {
    let fmt = format.or_else(|| ExportFormat::from_path(path))
        .unwrap_or(ExportFormat::Json);
    let content = format_tree(nodes, fmt);
    std::fs::write(path, content)
}

// ── Import (parse) ──────────────────────────────────────────────────────────

/// Parse flat data from a string. Returns column names + rows of field values.
pub fn parse_flat(content: &str, format: ExportFormat) -> Option<FlatExportData> {
    match format {
        ExportFormat::Json => parse_flat_json(content),
        ExportFormat::Yaml => parse_flat_yaml(content),
        ExportFormat::Ron  => parse_flat_ron(content),
        ExportFormat::Txt  => parse_flat_txt(content),
    }
}

/// Import flat data from file.
pub fn import_flat_from_file(path: &Path) -> Option<FlatExportData> {
    let format = ExportFormat::from_path(path)?;
    let content = std::fs::read_to_string(path).ok()?;
    parse_flat(&content, format)
}

// ═══════════════════════════════════════════════════════════════════════════
// ── JSON Formatter ──────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"'  => out.push_str("\\\""),
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
            if f.is_nan() { "null".into() }
            else if f.is_infinite() { "null".into() }
            else { format!("{}", f) }
        }
        FieldValue::Str(s) => format!("\"{}\"", json_escape(s)),
        FieldValue::Color(c) => format!("[{:.3}, {:.3}, {:.3}, {:.3}]", c[0], c[1], c[2], c[3]),
    }
}

fn format_flat_json(data: &FlatExportData) -> String {
    let mut out = String::from("[\n");
    for (ri, row) in data.rows.iter().enumerate() {
        out.push_str("  {");
        for (ci, val) in row.iter().enumerate() {
            if ci > 0 { out.push_str(", "); }
            let key = data.columns.get(ci).map(|s| s.as_str()).unwrap_or("?");
            out.push_str(&format!("\"{}\": {}", json_escape(key), field_value_json(val)));
        }
        out.push('}');
        if ri + 1 < data.rows.len() { out.push(','); }
        out.push('\n');
    }
    out.push(']');
    out
}

fn format_tree_json(nodes: &[TreeExportNode], indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let mut out = String::from("[\n");
    for (i, node) in nodes.iter().enumerate() {
        out.push_str(&format!("{}  {{\n", pad));
        for (fi, (key, val)) in node.fields.iter().enumerate() {
            out.push_str(&format!("{}  \"{}\": {}", pad1, json_escape(key), field_value_json(val)));
            if fi + 1 < node.fields.len() || !node.children.is_empty() { out.push(','); }
            out.push('\n');
        }
        if !node.children.is_empty() {
            out.push_str(&format!("{}  \"children\": {}\n", pad1,
                format_tree_json(&node.children, indent + 2)));
        }
        out.push_str(&format!("{}  }}", pad));
        if i + 1 < nodes.len() { out.push(','); }
        out.push('\n');
    }
    out.push_str(&format!("{}]", pad));
    out
}

// ── JSON Parser (minimal) ───────────────────────────────────────────────────

fn parse_flat_json(content: &str) -> Option<FlatExportData> {
    // Minimal JSON array-of-objects parser.
    let content = content.trim();
    if !content.starts_with('[') || !content.ends_with(']') { return None; }
    let inner = &content[1..content.len()-1];

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
        if escape { escape = false; continue; }
        if b == b'\\' && in_string { escape = true; continue; }
        if b == b'"' { in_string = !in_string; continue; }
        if in_string { continue; }

        if b == b'{' {
            if depth == 0 { start = Some(i); }
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
    if !s.starts_with('{') || !s.ends_with('}') { return Vec::new(); }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() { return Vec::new(); }

    let mut fields = Vec::new();
    let mut remaining = inner;

    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches([',', ' ', '\n', '\r', '\t']);
        if remaining.is_empty() { break; }

        // Parse key.
        if !remaining.starts_with('"') { break; }
        let key_end = remaining[1..].find('"').map(|p| p + 1);
        let Some(ke) = key_end else { break };
        let key = remaining[1..ke].to_string();
        remaining = &remaining[ke + 1..];

        // Skip colon.
        remaining = remaining.trim_start();
        if remaining.starts_with(':') { remaining = &remaining[1..]; }
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
    if s.starts_with('"') {
        // String value.
        let mut end = 1;
        let mut escape = false;
        for (i, b) in s[1..].bytes().enumerate() {
            if escape { escape = false; continue; }
            if b == b'\\' { escape = true; continue; }
            if b == b'"' { end = i + 1; break; }
        }
        let val = s[1..end].replace("\\\"", "\"").replace("\\n", "\n")
            .replace("\\t", "\t").replace("\\\\", "\\");
        (FieldValue::Str(val), &s[end + 1..])
    } else if s.starts_with("null") {
        (FieldValue::Null, &s[4..])
    } else if s.starts_with("true") {
        (FieldValue::Bool(true), &s[4..])
    } else if s.starts_with("false") {
        (FieldValue::Bool(false), &s[5..])
    } else if s.starts_with('[') {
        // Skip arrays (colors, children).
        let mut depth = 0i32;
        let mut end = 0;
        for (i, b) in s.bytes().enumerate() {
            if b == b'[' { depth += 1; }
            if b == b']' { depth -= 1; if depth == 0 { end = i + 1; break; } }
        }
        // Try parse as color [f32; 4].
        let arr_str = &s[1..end-1];
        let nums: Vec<f32> = arr_str.split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            (FieldValue::Color([nums[0], nums[1], nums[2], nums[3]]), &s[end..])
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

// ═══════════════════════════════════════════════════════════════════════════
// ── YAML Formatter ──────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

fn field_value_yaml(v: &FieldValue) -> String {
    match v {
        FieldValue::Null => "~".into(),
        FieldValue::Bool(b) => b.to_string(),
        FieldValue::Int(i) => i.to_string(),
        FieldValue::Float(f) => format!("{}", f),
        FieldValue::Str(s) => {
            if s.contains('\n') || s.contains(':') || s.contains('#')
                || s.starts_with(' ') || s.starts_with('"') || s.is_empty()
            {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                s.clone()
            }
        }
        FieldValue::Color(c) => format!("[{:.3}, {:.3}, {:.3}, {:.3}]", c[0], c[1], c[2], c[3]),
    }
}

fn format_flat_yaml(data: &FlatExportData) -> String {
    let mut out = String::new();
    for row in &data.rows {
        out.push_str("- ");
        for (ci, val) in row.iter().enumerate() {
            let key = data.columns.get(ci).map(|s| s.as_str()).unwrap_or("?");
            if ci == 0 {
                out.push_str(&format!("{}: {}\n", key, field_value_yaml(val)));
            } else {
                out.push_str(&format!("  {}: {}\n", key, field_value_yaml(val)));
            }
        }
    }
    out
}

fn format_tree_yaml(nodes: &[TreeExportNode], indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = String::new();
    for node in nodes {
        out.push_str(&format!("{}- ", pad));
        for (fi, (key, val)) in node.fields.iter().enumerate() {
            if fi == 0 {
                out.push_str(&format!("{}: {}\n", key, field_value_yaml(val)));
            } else {
                out.push_str(&format!("{}  {}: {}\n", pad, key, field_value_yaml(val)));
            }
        }
        if !node.children.is_empty() {
            out.push_str(&format!("{}  children:\n", pad));
            out.push_str(&format_tree_yaml(&node.children, indent + 2));
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════
// ── RON Formatter ───────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

fn field_value_ron(v: &FieldValue) -> String {
    match v {
        FieldValue::Null => "None".into(),
        FieldValue::Bool(b) => b.to_string(),
        FieldValue::Int(i) => i.to_string(),
        FieldValue::Float(f) => format!("{}", f),
        FieldValue::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        FieldValue::Color(c) => format!("({:.3}, {:.3}, {:.3}, {:.3})", c[0], c[1], c[2], c[3]),
    }
}

fn format_flat_ron(data: &FlatExportData) -> String {
    let mut out = String::from("[\n");
    for (ri, row) in data.rows.iter().enumerate() {
        out.push_str("  (");
        for (ci, val) in row.iter().enumerate() {
            if ci > 0 { out.push_str(", "); }
            let key = data.columns.get(ci).map(|s| s.as_str()).unwrap_or("?");
            out.push_str(&format!("{}: {}", key, field_value_ron(val)));
        }
        out.push(')');
        if ri + 1 < data.rows.len() { out.push(','); }
        out.push('\n');
    }
    out.push(']');
    out
}

fn format_tree_ron(nodes: &[TreeExportNode], indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = String::from("[\n");
    for (i, node) in nodes.iter().enumerate() {
        out.push_str(&format!("{}  (\n", pad));
        for (key, val) in &node.fields {
            out.push_str(&format!("{}    {}: {},\n", pad, key, field_value_ron(val)));
        }
        if !node.children.is_empty() {
            out.push_str(&format!("{}    children: {},\n", pad,
                format_tree_ron(&node.children, indent + 2)));
        }
        out.push_str(&format!("{}  )", pad));
        if i + 1 < nodes.len() { out.push(','); }
        out.push('\n');
    }
    out.push_str(&format!("{}]", pad));
    out
}

// ═══════════════════════════════════════════════════════════════════════════
// ── TXT Formatter (tab-separated) ───────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

fn format_flat_txt(data: &FlatExportData) -> String {
    let mut out = String::new();
    // Header.
    out.push_str(&data.columns.join("\t"));
    out.push('\n');
    // Rows.
    for row in &data.rows {
        let line: Vec<String> = row.iter().map(|v| v.to_string_lossy()).collect();
        out.push_str(&line.join("\t"));
        out.push('\n');
    }
    out
}

fn format_tree_txt(nodes: &[TreeExportNode], depth: usize) -> String {
    let mut out = String::new();
    let indent = "  ".repeat(depth);
    for node in nodes {
        let fields: Vec<String> = node.fields.iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_string_lossy()))
            .collect();
        out.push_str(&format!("{}{}\n", indent, fields.join(" | ")));
        if !node.children.is_empty() {
            out.push_str(&format_tree_txt(&node.children, depth + 1));
        }
    }
    out
}

fn parse_flat_txt(content: &str) -> Option<FlatExportData> {
    let mut lines = content.lines();
    let header = lines.next()?;
    let columns: Vec<String> = header.split('\t').map(|s| s.to_string()).collect();
    let mut rows = Vec::new();
    for line in lines {
        if line.trim().is_empty() { continue; }
        let vals: Vec<FieldValue> = line.split('\t')
            .map(|s| {
                let s = s.trim();
                if s.is_empty() { FieldValue::Null }
                else if s == "true" { FieldValue::Bool(true) }
                else if s == "false" { FieldValue::Bool(false) }
                else if let Ok(i) = s.parse::<i64>() { FieldValue::Int(i) }
                else if let Ok(f) = s.parse::<f64>() { FieldValue::Float(f) }
                else { FieldValue::Str(s.to_string()) }
            })
            .collect();
        rows.push(vals);
    }
    Some(FlatExportData { columns, rows })
}

// ═══════════════════════════════════════════════════════════════════════════
// ── YAML Parser ─────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

/// Parse a YAML list-of-objects into flat data.
///
/// Supports the subset generated by `format_flat_yaml`:
/// ```yaml
/// - name: Alice
///   age: 30
///   active: true
/// - name: Bob
///   age: 25
/// ```
fn parse_flat_yaml(content: &str) -> Option<FlatExportData> {
    let mut columns = Vec::new();
    let mut col_set = std::collections::HashSet::new();
    let mut rows: Vec<Vec<(String, FieldValue)>> = Vec::new();
    let mut current_fields: Vec<(String, FieldValue)> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') { continue; }

        if trimmed.starts_with("- ") {
            // New list item. Flush previous.
            if !current_fields.is_empty() {
                rows.push(std::mem::take(&mut current_fields));
            }
            // Parse the key: value on the same line as "- ".
            let rest = &trimmed[2..];
            if let Some((key, val)) = parse_yaml_kv(rest) {
                if col_set.insert(key.clone()) { columns.push(key.clone()); }
                current_fields.push((key, val));
            }
        } else if let Some((key, val)) = parse_yaml_kv(trimmed) {
            // Continuation field of current item.
            if col_set.insert(key.clone()) { columns.push(key.clone()); }
            current_fields.push((key, val));
        }
    }
    // Flush last item.
    if !current_fields.is_empty() {
        rows.push(current_fields);
    }

    if columns.is_empty() { return None; }

    // Build aligned rows.
    let mut data = FlatExportData::new(columns.clone());
    for fields in &rows {
        let mut row = vec![FieldValue::Null; columns.len()];
        for (key, val) in fields {
            if let Some(idx) = columns.iter().position(|c| c == key) {
                row[idx] = val.clone();
            }
        }
        data.add_row(row);
    }

    Some(data)
}

fn parse_yaml_kv(s: &str) -> Option<(String, FieldValue)> {
    let colon_pos = s.find(':')?;
    let key = s[..colon_pos].trim().to_string();
    let val_str = s[colon_pos + 1..].trim();

    let val = parse_yaml_value(val_str);
    Some((key, val))
}

fn parse_yaml_value(s: &str) -> FieldValue {
    if s.is_empty() || s == "~" || s == "null" { return FieldValue::Null; }
    if s == "true" { return FieldValue::Bool(true); }
    if s == "false" { return FieldValue::Bool(false); }

    // Quoted string.
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len()-1];
        return FieldValue::Str(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
    }

    // Array (color).
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len()-1];
        let nums: Vec<f32> = inner.split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            return FieldValue::Color([nums[0], nums[1], nums[2], nums[3]]);
        }
        return FieldValue::Str(s.to_string());
    }

    // Number.
    if let Ok(i) = s.parse::<i64>() { return FieldValue::Int(i); }
    if let Ok(f) = s.parse::<f64>() { return FieldValue::Float(f); }

    FieldValue::Str(s.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// ── RON Parser ──────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

/// Parse a RON list-of-tuples into flat data.
///
/// Supports the subset generated by `format_flat_ron`:
/// ```ron
/// [
///   (name: "Alice", age: 30, active: true),
///   (name: "Bob", age: 25, active: false),
/// ]
/// ```
fn parse_flat_ron(content: &str) -> Option<FlatExportData> {
    let content = content.trim();
    if !content.starts_with('[') || !content.ends_with(']') { return None; }
    let inner = &content[1..content.len()-1];

    let mut columns = Vec::new();
    let mut col_set = std::collections::HashSet::new();
    let mut rows = Vec::new();

    // Split RON tuples delimited by ( ... ).
    for tuple_str in split_ron_tuples(inner) {
        let fields = parse_ron_tuple(tuple_str.trim());
        if rows.is_empty() {
            for (key, _) in &fields {
                if col_set.insert(key.clone()) { columns.push(key.clone()); }
            }
        }
        rows.push(fields);
    }

    if columns.is_empty() { return None; }

    let mut data = FlatExportData::new(columns.clone());
    for fields in &rows {
        let mut row = vec![FieldValue::Null; columns.len()];
        for (key, val) in fields {
            if let Some(idx) = columns.iter().position(|c| c == key) {
                row[idx] = val.clone();
            }
        }
        data.add_row(row);
    }

    Some(data)
}

fn split_ron_tuples(s: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    let bytes = s.as_bytes();
    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape { escape = false; continue; }
        if b == b'\\' && in_string { escape = true; continue; }
        if b == b'"' { in_string = !in_string; continue; }
        if in_string { continue; }

        if b == b'(' {
            if depth == 0 { start = Some(i); }
            depth += 1;
        } else if b == b')' {
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

fn parse_ron_tuple(s: &str) -> Vec<(String, FieldValue)> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') { return Vec::new(); }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() { return Vec::new(); }

    let mut fields = Vec::new();
    let mut remaining = inner;

    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches([',', ' ', '\n', '\r', '\t']);
        if remaining.is_empty() { break; }

        // Parse key (unquoted identifier).
        let colon_pos = match remaining.find(':') {
            Some(p) => p,
            None => break,
        };
        let key = remaining[..colon_pos].trim().to_string();
        remaining = remaining[colon_pos + 1..].trim();

        // Parse value.
        let (val, rest) = parse_ron_value(remaining);
        fields.push((key, val));
        remaining = rest.trim_start_matches([',', ' ', '\n', '\r', '\t']);
    }

    fields
}

fn parse_ron_value(s: &str) -> (FieldValue, &str) {
    let s = s.trim();
    if s.starts_with('"') {
        // String.
        let mut end = 1;
        let mut escape = false;
        for (i, b) in s[1..].bytes().enumerate() {
            if escape { escape = false; continue; }
            if b == b'\\' { escape = true; continue; }
            if b == b'"' { end = i + 1; break; }
        }
        let val = s[1..end].replace("\\\"", "\"").replace("\\\\", "\\");
        (FieldValue::Str(val), &s[end + 1..])
    } else if s.starts_with("None") {
        (FieldValue::Null, &s[4..])
    } else if s.starts_with("true") {
        (FieldValue::Bool(true), &s[4..])
    } else if s.starts_with("false") {
        (FieldValue::Bool(false), &s[5..])
    } else if s.starts_with('(') {
        // Color tuple (r, g, b, a).
        let close = s.find(')').unwrap_or(s.len());
        let inner = &s[1..close];
        let nums: Vec<f32> = inner.split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            (FieldValue::Color([nums[0], nums[1], nums[2], nums[3]]), &s[close + 1..])
        } else {
            (FieldValue::Str(s[..close + 1].to_string()), &s[close + 1..])
        }
    } else {
        // Number.
        let end = s.find([',', ')', '\n', ' ']).unwrap_or(s.len());
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

// ═══════════════════════════════════════════════════════════════════════════
// ── ExportDialog ────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

/// Selection scope for export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportScope {
    /// Export only selected rows/nodes.
    #[default]
    Selected,
    /// Export all rows/nodes.
    All,
}

/// Configuration for optional export/import support.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Whether export is enabled.
    pub enable_export: bool,
    /// Whether import is enabled.
    pub enable_import: bool,
    /// Default export format.
    pub default_format: ExportFormat,
    /// Available formats (user can choose).
    pub formats: Vec<ExportFormat>,
    /// Default scope (selected vs all).
    pub default_scope: ExportScope,
    /// Default export filename (without extension).
    pub default_filename: String,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            enable_export: false,
            enable_import: false,
            default_format: ExportFormat::Json,
            formats: ExportFormat::ALL.to_vec(),
            default_scope: ExportScope::Selected,
            default_filename: "export".to_string(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_flat() -> FlatExportData {
        let mut data = FlatExportData::new(vec!["name".into(), "age".into(), "active".into()]);
        data.add_row(vec![
            FieldValue::Str("Alice".into()), FieldValue::Int(30), FieldValue::Bool(true),
        ]);
        data.add_row(vec![
            FieldValue::Str("Bob".into()), FieldValue::Int(25), FieldValue::Bool(false),
        ]);
        data
    }

    fn sample_tree() -> Vec<TreeExportNode> {
        vec![
            TreeExportNode {
                fields: vec![
                    ("name".into(), FieldValue::Str("Root".into())),
                    ("value".into(), FieldValue::Int(100)),
                ],
                children: vec![
                    TreeExportNode {
                        fields: vec![
                            ("name".into(), FieldValue::Str("Child A".into())),
                            ("value".into(), FieldValue::Int(50)),
                        ],
                        children: vec![
                            TreeExportNode {
                                fields: vec![
                                    ("name".into(), FieldValue::Str("Grandchild".into())),
                                    ("value".into(), FieldValue::Int(10)),
                                ],
                                children: vec![],
                            },
                        ],
                    },
                    TreeExportNode {
                        fields: vec![
                            ("name".into(), FieldValue::Str("Child B".into())),
                            ("value".into(), FieldValue::Float(3.14)),
                        ],
                        children: vec![],
                    },
                ],
            },
        ]
    }

    #[test]
    fn test_flat_json() {
        let data = sample_flat();
        let json = format_flat(&data, ExportFormat::Json);
        assert!(json.contains("\"name\": \"Alice\""));
        assert!(json.contains("\"age\": 30"));
        assert!(json.contains("\"active\": true"));
        assert!(json.contains("\"name\": \"Bob\""));
    }

    #[test]
    fn test_flat_json_roundtrip() {
        let data = sample_flat();
        let json = format_flat(&data, ExportFormat::Json);
        let parsed = parse_flat(&json, ExportFormat::Json).unwrap();
        assert_eq!(parsed.columns, data.columns);
        assert_eq!(parsed.rows.len(), 2);
    }

    #[test]
    fn test_flat_yaml() {
        let data = sample_flat();
        let yaml = format_flat(&data, ExportFormat::Yaml);
        assert!(yaml.contains("name: Alice"));
        assert!(yaml.contains("age: 30"));
    }

    #[test]
    fn test_flat_ron() {
        let data = sample_flat();
        let ron = format_flat(&data, ExportFormat::Ron);
        assert!(ron.contains("name: \"Alice\""));
        assert!(ron.contains("age: 30"));
    }

    #[test]
    fn test_flat_txt() {
        let data = sample_flat();
        let txt = format_flat(&data, ExportFormat::Txt);
        assert!(txt.starts_with("name\tage\tactive\n"));
        assert!(txt.contains("Alice\t30\ttrue"));
    }

    #[test]
    fn test_flat_txt_roundtrip() {
        let data = sample_flat();
        let txt = format_flat(&data, ExportFormat::Txt);
        let parsed = parse_flat(&txt, ExportFormat::Txt).unwrap();
        assert_eq!(parsed.columns, data.columns);
        assert_eq!(parsed.rows.len(), 2);
    }

    #[test]
    fn test_tree_json() {
        let nodes = sample_tree();
        let json = format_tree(&nodes, ExportFormat::Json);
        assert!(json.contains("\"name\": \"Root\""));
        assert!(json.contains("\"children\":"));
        assert!(json.contains("\"name\": \"Grandchild\""));
    }

    #[test]
    fn test_tree_yaml() {
        let nodes = sample_tree();
        let yaml = format_tree(&nodes, ExportFormat::Yaml);
        assert!(yaml.contains("name: Root"));
        assert!(yaml.contains("children:"));
        assert!(yaml.contains("name: Grandchild"));
    }

    #[test]
    fn test_tree_ron() {
        let nodes = sample_tree();
        let ron = format_tree(&nodes, ExportFormat::Ron);
        assert!(ron.contains("name: \"Root\""));
        assert!(ron.contains("children:"));
    }

    #[test]
    fn test_tree_txt() {
        let nodes = sample_tree();
        let txt = format_tree(&nodes, ExportFormat::Txt);
        assert!(txt.contains("name: Root"));
        assert!(txt.contains("  name: Child A")); // indented
        assert!(txt.contains("    name: Grandchild")); // double indented
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(ExportFormat::from_extension("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::from_extension("yaml"), Some(ExportFormat::Yaml));
        assert_eq!(ExportFormat::from_extension("yml"), Some(ExportFormat::Yaml));
        assert_eq!(ExportFormat::from_extension("ron"), Some(ExportFormat::Ron));
        assert_eq!(ExportFormat::from_extension("txt"), Some(ExportFormat::Txt));
        assert_eq!(ExportFormat::from_extension("csv"), Some(ExportFormat::Txt));
        assert_eq!(ExportFormat::from_extension("xyz"), None);
    }

    #[test]
    fn test_field_value_display() {
        assert_eq!(FieldValue::Null.to_string_lossy(), "");
        assert_eq!(FieldValue::Bool(true).to_string_lossy(), "true");
        assert_eq!(FieldValue::Int(42).to_string_lossy(), "42");
        assert_eq!(FieldValue::Str("hello".into()).to_string_lossy(), "hello");
    }

    #[test]
    fn test_json_escape() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape("he\"llo"), "he\\\"llo");
        assert_eq!(json_escape("line\nnew"), "line\\nnew");
    }

    #[test]
    fn test_export_config_default() {
        let cfg = ExportConfig::default();
        assert!(!cfg.enable_export);
        assert!(!cfg.enable_import);
        assert_eq!(cfg.default_format, ExportFormat::Json);
    }

    // ── YAML roundtrip ──────────────────────────────────────────

    #[test]
    fn test_yaml_roundtrip() {
        let data = sample_flat();
        let yaml = format_flat(&data, ExportFormat::Yaml);
        let parsed = parse_flat(&yaml, ExportFormat::Yaml).unwrap();
        assert_eq!(parsed.columns, data.columns);
        assert_eq!(parsed.rows.len(), 2);
        // Check first row values.
        match &parsed.rows[0][0] {
            FieldValue::Str(s) => assert_eq!(s, "Alice"),
            other => panic!("Expected Str, got {:?}", other),
        }
        match &parsed.rows[0][1] {
            FieldValue::Int(i) => assert_eq!(*i, 30),
            other => panic!("Expected Int, got {:?}", other),
        }
        match &parsed.rows[0][2] {
            FieldValue::Bool(b) => assert!(*b),
            other => panic!("Expected Bool(true), got {:?}", other),
        }
    }

    #[test]
    fn test_yaml_parse_quoted_strings() {
        let yaml = "- name: \"hello: world\"\n  value: 42\n";
        let parsed = parse_flat_yaml(yaml).unwrap();
        assert_eq!(parsed.rows.len(), 1);
        match &parsed.rows[0][0] {
            FieldValue::Str(s) => assert_eq!(s, "hello: world"),
            other => panic!("Expected Str, got {:?}", other),
        }
    }

    // ── RON roundtrip ───────────────────────────────────────────

    #[test]
    fn test_ron_roundtrip() {
        let data = sample_flat();
        let ron = format_flat(&data, ExportFormat::Ron);
        let parsed = parse_flat(&ron, ExportFormat::Ron).unwrap();
        assert_eq!(parsed.columns, data.columns);
        assert_eq!(parsed.rows.len(), 2);
        match &parsed.rows[0][0] {
            FieldValue::Str(s) => assert_eq!(s, "Alice"),
            other => panic!("Expected Str, got {:?}", other),
        }
        match &parsed.rows[1][1] {
            FieldValue::Int(i) => assert_eq!(*i, 25),
            other => panic!("Expected Int, got {:?}", other),
        }
    }

    #[test]
    fn test_ron_parse_none_and_float() {
        let ron = "[\n  (x: None, y: 3.14),\n]";
        let parsed = parse_flat_ron(ron).unwrap();
        assert_eq!(parsed.rows.len(), 1);
        assert!(matches!(parsed.rows[0][0], FieldValue::Null));
        match &parsed.rows[0][1] {
            FieldValue::Float(f) => assert!((*f - 3.14).abs() < 0.001),
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn test_all_formats_roundtrip() {
        let data = sample_flat();
        for fmt in ExportFormat::ALL {
            let exported = format_flat(&data, *fmt);
            let parsed = parse_flat(&exported, *fmt);
            assert!(parsed.is_some(), "Failed to parse {:?} roundtrip", fmt);
            let parsed = parsed.unwrap();
            assert_eq!(parsed.rows.len(), 2, "Wrong row count for {:?}", fmt);
        }
    }
}
