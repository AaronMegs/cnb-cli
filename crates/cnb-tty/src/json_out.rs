//! JSON output helpers.

use std::io::Write;

use serde_json::Value;

use crate::TtyError;

/// Write a JSON value to the given writer.
///
/// `pretty=true` produces 2-space-indented output with a trailing newline.
pub fn write_json<W: Write>(w: &mut W, value: &Value, pretty: bool) -> Result<(), TtyError> {
    if pretty {
        serde_json::to_writer_pretty(&mut *w, value).map_err(|e| TtyError::Template(e.to_string()))?;
    } else {
        serde_json::to_writer(&mut *w, value).map_err(|e| TtyError::Template(e.to_string()))?;
    }
    writeln!(w)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_includes_newline() {
        let mut buf = Vec::new();
        write_json(&mut buf, &serde_json::json!({"a":1}), true).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));
        assert!(s.contains("  \"a\""));
    }

    #[test]
    fn compact_one_line() {
        let mut buf = Vec::new();
        write_json(&mut buf, &serde_json::json!({"a":1,"b":2}), false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s.trim(), "{\"a\":1,\"b\":2}");
    }
}
