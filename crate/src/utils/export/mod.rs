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
mod model;
mod ron;
mod txt;
mod yaml;

use std::path::Path;

// Re-export the data model so external paths like
// `crate::utils::export::Exportable` / `::FieldValue` stay valid after the
// type/trait definitions moved into the `model` sibling (mod.rs < 500 lines).
pub use model::{
    ExportConfig, ExportFormat, ExportScope, Exportable, FieldValue, FlatExportData, Importable,
    TreeExportNode,
};

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

    #[test]
    fn extension_and_display_name_cover_all() {
        // Extension and display name are 1:1 with the variant; round-trip
        // each through `from_extension`.
        for fmt in ExportFormat::ALL {
            assert_eq!(ExportFormat::from_extension(fmt.extension()), Some(*fmt));
            assert!(!fmt.display_name().is_empty());
        }
    }

    #[test]
    fn from_path_uses_extension() {
        use std::path::Path;
        assert_eq!(
            ExportFormat::from_path(Path::new("/tmp/data.yaml")),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("a/b/c.RON")),
            Some(ExportFormat::Ron)
        );
        assert_eq!(ExportFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn export_then_import_file_round_trips() {
        // Exercise the file-IO helpers end to end via a temp file.
        let data = sample_flat();
        let mut path = std::env::temp_dir();
        let unique = format!(
            "utils_export_test_{}_{:?}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        path.push(unique);

        export_flat_to_file(&data, &path, None).expect("write");
        let imported = import_flat_from_file(&path).expect("read");
        assert_eq!(imported.columns, data.columns);
        assert_eq!(imported.rows.len(), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_flat_rejects_garbage_for_structured_formats() {
        assert!(parse_flat("not json", ExportFormat::Json).is_none());
        assert!(parse_flat("not ron", ExportFormat::Ron).is_none());
    }

    #[test]
    fn malformed_input_never_panics() {
        // Fuzz-lite: a handful of broken payloads must return cleanly,
        // never panic (regression for the slice-underflow / OOB bugs).
        let broken = [
            "[{\"a\": [1, 2, 3", // unterminated array
            "[{\"a\": \"unterminated",
            "[(x: (0.1, 0.2", // unterminated RON color
            "[(name: \"oops", // unterminated RON string
        ];
        for s in broken {
            for fmt in ExportFormat::ALL {
                // Just must not panic; result is don't-care.
                let _ = parse_flat(s, *fmt);
            }
        }
    }
}
