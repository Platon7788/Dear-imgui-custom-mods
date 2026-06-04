//! Data model for the export/import system: format enum, typed field
//! values, the [`Exportable`] / [`Importable`] traits, the flat-row and
//! tree containers, and the export configuration / scope types.
//!
//! These are the format-agnostic building blocks; the per-format
//! (de)serialisers live in the `json` / `yaml` / `ron` / `txt` siblings
//! and the dispatch + file-IO helpers in [`super`]. Public items are
//! re-exported from `super` so external paths such as
//! `crate::utils::export::Exportable` stay valid.

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
