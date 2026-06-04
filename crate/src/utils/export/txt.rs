//! Tab-separated text formatter + parser for [`FlatExportData`] /
//! [`TreeExportNode`]. The "least common denominator" format — survives
//! copy-paste into spreadsheets / terminals.

use super::{FieldValue, FlatExportData, TreeExportNode};

/// TSV is a structural format: a literal tab, carriage return, or newline
/// inside a cell would silently spill into an extra column / row and
/// corrupt the table. There is no portable TSV escape, so we replace those
/// control characters with a single space — lossy but structurally safe,
/// which matters more for a "paste into a spreadsheet" format than exact
/// fidelity of embedded whitespace.
fn tsv_cell(s: String) -> String {
    if s.contains(['\t', '\n', '\r']) {
        s.replace(['\t', '\n', '\r'], " ")
    } else {
        s
    }
}

pub(super) fn format_flat(data: &FlatExportData) -> String {
    let mut out = String::new();
    // Header.
    let header: Vec<String> = data.columns.iter().map(|c| tsv_cell(c.clone())).collect();
    out.push_str(&header.join("\t"));
    out.push('\n');
    // Rows.
    for row in &data.rows {
        let line: Vec<String> = row.iter().map(|v| tsv_cell(v.to_string_lossy())).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tab_does_not_spill_into_extra_column() {
        // Regression: a tab inside a cell used to add a phantom column.
        let mut d = FlatExportData::new(vec!["a".into(), "b".into()]);
        d.add_row(vec![FieldValue::Str("x\ty".into()), FieldValue::Int(1)]);
        let txt = format_flat(&d);
        // Every data line must have exactly one separator (two columns).
        for line in txt.lines() {
            assert_eq!(line.matches('\t').count(), 1, "line {line:?} broke columns");
        }
        let parsed = parse_flat(&txt).unwrap();
        assert_eq!(parsed.columns.len(), 2);
        assert_eq!(parsed.rows[0].len(), 2);
    }

    #[test]
    fn embedded_newline_does_not_spill_into_extra_row() {
        let mut d = FlatExportData::new(vec!["a".into()]);
        d.add_row(vec![FieldValue::Str("line1\nline2".into())]);
        let txt = format_flat(&d);
        let parsed = parse_flat(&txt).unwrap();
        assert_eq!(parsed.rows.len(), 1, "newline created a phantom row");
    }

    #[test]
    fn round_trip_scalar_types() {
        let mut d = FlatExportData::new(vec!["s".into(), "i".into(), "b".into()]);
        d.add_row(vec![
            FieldValue::Str("hello".into()),
            FieldValue::Int(7),
            FieldValue::Bool(true),
        ]);
        let txt = format_flat(&d);
        let parsed = parse_flat(&txt).unwrap();
        assert!(matches!(&parsed.rows[0][0], FieldValue::Str(s) if s == "hello"));
        assert!(matches!(parsed.rows[0][1], FieldValue::Int(7)));
        assert!(matches!(parsed.rows[0][2], FieldValue::Bool(true)));
    }

    #[test]
    fn empty_cell_parses_as_null() {
        let txt = "a\tb\n\t5\n";
        let parsed = parse_flat(txt).unwrap();
        assert!(matches!(parsed.rows[0][0], FieldValue::Null));
        assert!(matches!(parsed.rows[0][1], FieldValue::Int(5)));
    }
}
