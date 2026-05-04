//! Export/import system for VirtualTable and VirtualTree data.
//!
//! Provides format-agnostic serialization traits and built-in formatters
//! for JSON, YAML, RON, and TXT. No external dependencies — pure Rust.
//!
//! ## Module layout
//!
//! ```text
//! utils/export/
//! ├── mod.rs   — types + traits + dispatch (`format_flat`, `parse_flat`)
//! ├── json.rs  — JSON formatter + minimal parser
//! ├── yaml.rs  — YAML subset formatter + parser
//! ├── ron.rs   — RON subset formatter + parser
//! └── txt.rs   — tab-separated text formatter + parser
//! ```
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

mod json;
mod ron;
mod txt;
mod yaml;

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
    pub const ALL: &'static [ExportFormat] = &[Self::Json, Self::Yaml, Self::Ron, Self::Txt];

    /// File extension (without dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Ron => "ron",
            Self::Txt => "txt",
        }
    }

    /// Display name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Ron => "RON",
            Self::Txt => "Text",
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
    fn field_count() -> usize {
        Self::field_names().len()
    }
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
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<FieldValue>) {
        self.rows.push(row);
    }
}

// ── Format dispatch ─────────────────────────────────────────────────────────

/// Format flat table data to string.
pub fn format_flat(data: &FlatExportData, format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => json::format_flat(data),
        ExportFormat::Yaml => yaml::format_flat(data),
        ExportFormat::Ron => ron::format_flat(data),
        ExportFormat::Txt => txt::format_flat(data),
    }
}

/// Format hierarchical tree data to string.
pub fn format_tree(nodes: &[TreeExportNode], format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => json::format_tree(nodes, 0),
        ExportFormat::Yaml => yaml::format_tree(nodes, 0),
        ExportFormat::Ron => ron::format_tree(nodes, 0),
        ExportFormat::Txt => txt::format_tree(nodes, 0),
    }
}

/// Export flat data to file.
pub fn export_flat_to_file(
    data: &FlatExportData,
    path: &Path,
    format: Option<ExportFormat>,
) -> std::io::Result<()> {
    let fmt = format
        .or_else(|| ExportFormat::from_path(path))
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
    let fmt = format
        .or_else(|| ExportFormat::from_path(path))
        .unwrap_or(ExportFormat::Json);
    let content = format_tree(nodes, fmt);
    std::fs::write(path, content)
}

/// Parse flat data from a string. Returns column names + rows of field values.
pub fn parse_flat(content: &str, format: ExportFormat) -> Option<FlatExportData> {
    match format {
        ExportFormat::Json => json::parse_flat(content),
        ExportFormat::Yaml => yaml::parse_flat(content),
        ExportFormat::Ron => ron::parse_flat(content),
        ExportFormat::Txt => txt::parse_flat(content),
    }
}

/// Import flat data from file.
pub fn import_flat_from_file(path: &Path) -> Option<FlatExportData> {
    let format = ExportFormat::from_path(path)?;
    let content = std::fs::read_to_string(path).ok()?;
    parse_flat(&content, format)
}

// ── Export config / scope ───────────────────────────────────────────────────

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
            FieldValue::Str("Alice".into()),
            FieldValue::Int(30),
            FieldValue::Bool(true),
        ]);
        data.add_row(vec![
            FieldValue::Str("Bob".into()),
            FieldValue::Int(25),
            FieldValue::Bool(false),
        ]);
        data
    }

    fn sample_tree() -> Vec<TreeExportNode> {
        vec![TreeExportNode {
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
                    children: vec![TreeExportNode {
                        fields: vec![
                            ("name".into(), FieldValue::Str("Grandchild".into())),
                            ("value".into(), FieldValue::Int(10)),
                        ],
                        children: vec![],
                    }],
                },
                TreeExportNode {
                    fields: vec![
                        ("name".into(), FieldValue::Str("Child B".into())),
                        ("value".into(), FieldValue::Float(3.25)),
                    ],
                    children: vec![],
                },
            ],
        }]
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
        assert_eq!(
            ExportFormat::from_extension("json"),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_extension("yaml"),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(
            ExportFormat::from_extension("yml"),
            Some(ExportFormat::Yaml)
        );
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
    fn test_export_config_default() {
        let cfg = ExportConfig::default();
        assert!(!cfg.enable_export);
        assert!(!cfg.enable_import);
        assert_eq!(cfg.default_format, ExportFormat::Json);
    }

    #[test]
    fn test_yaml_roundtrip() {
        let data = sample_flat();
        let yaml = format_flat(&data, ExportFormat::Yaml);
        let parsed = parse_flat(&yaml, ExportFormat::Yaml).unwrap();
        assert_eq!(parsed.columns, data.columns);
        assert_eq!(parsed.rows.len(), 2);
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
        let parsed = parse_flat(yaml, ExportFormat::Yaml).unwrap();
        assert_eq!(parsed.rows.len(), 1);
        match &parsed.rows[0][0] {
            FieldValue::Str(s) => assert_eq!(s, "hello: world"),
            other => panic!("Expected Str, got {:?}", other),
        }
    }

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
        let ron = "[\n  (x: None, y: 3.25),\n]";
        let parsed = parse_flat(ron, ExportFormat::Ron).unwrap();
        assert_eq!(parsed.rows.len(), 1);
        assert!(matches!(parsed.rows[0][0], FieldValue::Null));
        match &parsed.rows[0][1] {
            FieldValue::Float(f) => assert!((*f - 3.25).abs() < 0.001),
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
