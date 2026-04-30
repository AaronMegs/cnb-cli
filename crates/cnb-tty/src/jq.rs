//! Apply a jq filter to a JSON value using the pure-Rust `jaq` interpreter.

use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use serde_json::Value;

use crate::TtyError;

/// Apply a jq expression to `input`. Returns all output values.
pub fn apply(input: &Value, expr: &str) -> Result<Vec<Value>, TtyError> {
    // Parse + compile.
    let (parsed, errs) = jaq_parse::parse(expr, jaq_parse::main());
    if !errs.is_empty() {
        let msg = errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
        return Err(TtyError::JqParse(msg));
    }
    let parsed = parsed.ok_or_else(|| TtyError::JqParse("empty filter".into()))?;

    let mut ctx = ParseCtx::new(Vec::new());
    ctx.insert_natives(jaq_core::core());
    ctx.insert_defs(jaq_std::std());
    let filter = ctx.compile(parsed);
    if !ctx.errs.is_empty() {
        let msg = ctx
            .errs
            .iter()
            .map(|(e, _)| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(TtyError::JqParse(msg));
    }

    let inputs = RcIter::new(core::iter::empty());
    let mut out = Vec::new();
    for v in filter.run((Ctx::new([], &inputs), Val::from(input.clone()))) {
        match v {
            Ok(v) => out.push(Value::from(v)),
            Err(e) => return Err(TtyError::JqRun(e.to_string())),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identity() {
        let v = json!({"a": 1});
        let out = apply(&v, ".").unwrap();
        assert_eq!(out, vec![v]);
    }

    #[test]
    fn field_access() {
        let v = json!({"name": "alice"});
        let out = apply(&v, ".name").unwrap();
        assert_eq!(out, vec![json!("alice")]);
    }

    #[test]
    fn array_iteration() {
        let v = json!([{"n":1},{"n":2}]);
        let out = apply(&v, ".[] | .n").unwrap();
        assert_eq!(out, vec![json!(1), json!(2)]);
    }

    #[test]
    fn parse_error() {
        let v = json!({});
        let err = apply(&v, "((((").unwrap_err();
        assert!(matches!(err, TtyError::JqParse(_)));
    }
}
