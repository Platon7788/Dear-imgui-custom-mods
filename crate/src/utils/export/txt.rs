//! Tab-separated text formatter + parser for [`FlatExportData`] /
//! [`TreeExportNode`]. The "least common denominator" format — survives
//! copy-paste into spreadsheets / terminals.

use super::{FieldValue, FlatExportData, TreeExportNode};

pub(super) fn format_flat(data: &FlatExportData) -> String {
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

pub(super) fn format_tree(nodes: &[TreeExportNode], depth: usize) -> String {
    let mut out = String::new();
    let indent = "  ".repeat(depth);
    for node in nodes {
        let fields: Vec<String> = node
            .fields
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_string_lossy()))
            .collect();
        out.push_str(&format!("{}{}\n", indent, fields.join(" | ")));
        if !node.children.is_empty() {
            out.push_str(&format_tree(&node.children, depth + 1));
        }
    }
    out
}

pub(super) fn parse_flat(content: &str) -> Option<FlatExportData> {
    let mut lines = content.lines();
    let header = lines.next()?;
    let columns: Vec<String> = header.split('\t').map(|s| s.to_string()).collect();
    let mut rows = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let vals: Vec<FieldValue> = line
            .split('\t')
            .map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    FieldValue::Null
                } else if s == "true" {
                    FieldValue::Bool(true)
                } else if s == "false" {
                    FieldValue::Bool(false)
                } else if let Ok(i) = s.parse::<i64>() {
                    FieldValue::Int(i)
                } else if let Ok(f) = s.parse::<f64>() {
                    FieldValue::Float(f)
                } else {
                    FieldValue::Str(s.to_string())
                }
            })
            .collect();
        rows.push(vals);
    }
    Some(FlatExportData { columns, rows })
}
