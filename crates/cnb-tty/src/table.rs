//! Plain-text table rendering for `repo list` / `issue list` / `pr list`.
//!
//! TTY-aware: when stdout is a TTY we draw bordered ASCII tables; when piped
//! we emit tab-separated rows so callers can pipe into `cut`, `awk`, `column`,
//! etc. (matches `gh` behavior).
//!
//! This is intentionally a thin wrapper over `comfy-table` so we keep one
//! dependency surface for tables across all command modules.

use std::io::Write;

use comfy_table::{ContentArrangement, Table};

use crate::TtyError;

/// Render rows as a table (TTY) or TSV (piped) and write to `w`.
///
/// - `headers`: column titles in upper-case (`gh` convention: `NAME`, `STATUS`).
/// - `rows`: each row must have `headers.len()` cells. Extra/missing cells are
///   right-padded with empty strings (defensive — never panics).
/// - `tty`: pass `IoStreams::stdout_is_tty`. When `false`, emit TSV and skip
///   the header row (so the output is fully scriptable).
pub fn write_table<W: Write>(w: &mut W, headers: &[&str], rows: &[Vec<String>], tty: bool) -> Result<(), TtyError> {
    if !tty {
        // Scriptable: TSV, no header (gh-style).
        for row in rows {
            let mut first = true;
            for i in 0..headers.len() {
                if !first {
                    write!(w, "\t")?;
                }
                first = false;
                let cell = row.get(i).map_or("", String::as_str);
                // Replace embedded tabs/newlines so each row stays one line.
                let safe = cell.replace(['\t', '\n', '\r'], " ");
                write!(w, "{safe}")?;
            }
            writeln!(w)?;
        }
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(headers.iter().copied().map(comfy_table::Cell::new));
    for row in rows {
        let cells: Vec<comfy_table::Cell> = (0..headers.len())
            .map(|i| comfy_table::Cell::new(row.get(i).map_or("", String::as_str)))
            .collect();
        table.add_row(cells);
    }
    writeln!(w, "{table}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_when_not_tty() {
        let mut buf = Vec::new();
        write_table(
            &mut buf,
            &["NAME", "DESC"],
            &[
                vec!["foo".into(), "a repo".into()],
                vec!["bar".into(), "another".into()],
            ],
            false,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "foo\ta repo\nbar\tanother\n");
    }

    #[test]
    fn tsv_replaces_embedded_tabs_and_newlines() {
        let mut buf = Vec::new();
        write_table(
            &mut buf,
            &["A", "B"],
            &[vec!["x\ty".into(), "first\nsecond".into()]],
            false,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "x y\tfirst second\n");
    }

    #[test]
    fn tty_includes_headers() {
        let mut buf = Vec::new();
        write_table(&mut buf, &["NAME"], &[vec!["foo".into()]], true).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("NAME"));
        assert!(s.contains("foo"));
    }

    #[test]
    fn missing_cells_padded_safely() {
        let mut buf = Vec::new();
        write_table(&mut buf, &["A", "B", "C"], &[vec!["x".into()]], false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "x\t\t\n");
    }
}
