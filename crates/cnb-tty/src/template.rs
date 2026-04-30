//! Render a JSON value with a `tinytemplate` template string.

use serde_json::Value;
use tinytemplate::TinyTemplate;

use crate::TtyError;

/// Render `value` against `tpl`. Template name is fixed to `"_"`.
pub fn apply(value: &Value, tpl: &str) -> Result<String, TtyError> {
    let mut tt = TinyTemplate::new();
    tt.add_template("_", tpl)
        .map_err(|e| TtyError::Template(e.to_string()))?;
    tt.render("_", value).map_err(|e| TtyError::Template(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_simple_field() {
        let v = json!({"name":"alice","age":30});
        let s = apply(&v, "{name} is {age}").unwrap();
        assert_eq!(s, "alice is 30");
    }

    #[test]
    fn invalid_template_errors() {
        let v = json!({});
        let err = apply(&v, "{unclosed").unwrap_err();
        assert!(matches!(err, TtyError::Template(_)));
    }
}
