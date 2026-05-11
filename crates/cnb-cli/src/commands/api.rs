//! `cnb api` — generic REST passthrough aligned with `gh api`.

use clap::Args;
use cnb_tty::{jq, json_out, template};
use reqwest::Method;
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;
use crate::http::{passthrough, sensitive};

#[derive(Debug, Args)]
pub struct ApiArgs {
    /// Request path, e.g. `/user` or `/cnb/feedback/-/issues`.
    pub path: String,

    /// HTTP method (default: GET, or POST when fields are present).
    #[arg(short = 'X', long = "method")]
    pub method: Option<String>,

    /// Add a body field as `key=value` (multiple allowed).
    #[arg(short = 'f', long = "field", value_name = "key=value")]
    pub fields: Vec<String>,

    /// Read a body field value from a file: `key=@path` or `key=-` for stdin.
    #[arg(short = 'F', long = "field-file", value_name = "key=@file")]
    pub field_files: Vec<String>,

    /// Add a request header (`-H "Name: Value"`, multiple allowed).
    #[arg(short = 'H', long = "header", value_name = "Name: Value")]
    pub headers: Vec<String>,

    /// Print response headers in addition to the body.
    #[arg(short = 'i', long)]
    pub include: bool,

    /// Suppress the response body (only the exit code matters).
    #[arg(long)]
    pub silent: bool,

    /// jq filter applied to the JSON response.
    #[arg(long)]
    pub jq: Option<String>,

    /// tinytemplate string applied to the JSON response.
    #[arg(long)]
    pub template: Option<String>,

    /// Auto-paginate (M1: not implemented).
    #[arg(long)]
    pub paginate: bool,
}

pub async fn run(ctx: &mut Context, args: ApiArgs) -> Result<(), CliError> {
    if args.paginate {
        return Err(CliError::NotImplemented("--paginate (planned in M2)".into()));
    }

    let body = build_body(&args.fields, &args.field_files)?;
    let method = pick_method(args.method.as_deref(), body.is_some())?;
    let headers = parse_headers(&args.headers)?;

    let resp = passthrough::request(ctx, method, &args.path, body, &headers).await?;

    if args.include {
        println!("HTTP/1.1 {}", resp.status);
        for (k, v) in &resp.headers {
            // Per DESIGN §6: never print sensitive headers verbatim. The server's
            // own response headers are not secrets; we still redact set-cookie and
            // anything that looks like a token to be safe.
            let printed = if sensitive::is_sensitive(k) {
                "***".to_owned()
            } else {
                v.clone()
            };
            println!("{k}: {printed}");
        }
        println!();
    }

    if !resp.is_success() {
        // For consistent error mapping with the rest of the CLI, surface the
        // structured error here (unless --silent).
        return Err(passthrough::into_error(resp));
    }

    if args.silent {
        return Ok(());
    }

    if let Some(tpl) = args.template.as_deref() {
        let v: Value = serde_json::from_str(&resp.body).unwrap_or(Value::String(resp.body.clone()));
        let s = template::apply(&v, tpl)?;
        println!("{s}");
        return Ok(());
    }

    if let Some(expr) = args.jq.as_deref() {
        let v: Value = serde_json::from_str(&resp.body).unwrap_or(Value::String(resp.body.clone()));
        let outs = jq::apply(&v, expr)?;
        let mut stdout = std::io::stdout().lock();
        for o in outs {
            json_out::write_json(&mut stdout, &o, false)?;
        }
        return Ok(());
    }

    // Default: pretty-print if JSON, else echo verbatim.
    match serde_json::from_str::<Value>(&resp.body) {
        Ok(v) => {
            let mut stdout = std::io::stdout().lock();
            json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
        }
        Err(_) => {
            println!("{}", resp.body);
        }
    }
    Ok(())
}

fn pick_method(explicit: Option<&str>, has_body: bool) -> Result<Method, CliError> {
    if let Some(m) = explicit {
        return Method::from_bytes(m.to_uppercase().as_bytes())
            .map_err(|_| CliError::BadArgs(format!("invalid method: {m}")));
    }
    Ok(if has_body { Method::POST } else { Method::GET })
}

fn parse_headers(raw: &[String]) -> Result<Vec<(String, String)>, CliError> {
    raw.iter()
        .map(|h| {
            let (name, value) = h
                .split_once(':')
                .ok_or_else(|| CliError::BadArgs(format!("bad header (expected `Name: Value`): {h}")))?;
            Ok((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn build_body(fields: &[String], field_files: &[String]) -> Result<Option<Value>, CliError> {
    if fields.is_empty() && field_files.is_empty() {
        return Ok(None);
    }
    let mut map = serde_json::Map::new();

    for f in fields {
        let (k, v) = f
            .split_once('=')
            .ok_or_else(|| CliError::BadArgs(format!("bad field (expected `key=value`): {f}")))?;
        map.insert(k.to_owned(), Value::String(v.to_owned()));
    }

    for f in field_files {
        let (k, spec) = f
            .split_once('=')
            .ok_or_else(|| CliError::BadArgs(format!("bad field-file (expected `key=@path`): {f}")))?;
        let path = spec
            .strip_prefix('@')
            .ok_or_else(|| CliError::BadArgs(format!("field-file value must start with @: {f}")))?;
        let raw = if path == "-" {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
            s
        } else {
            std::fs::read_to_string(path)?
        };
        // Try to parse as JSON; otherwise insert as string.
        let v: Value = serde_json::from_str(&raw).unwrap_or(Value::String(raw));
        map.insert(k.to_owned(), v);
    }
    Ok(Some(Value::Object(map)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_method_defaults() {
        assert_eq!(pick_method(None, false).unwrap(), Method::GET);
        assert_eq!(pick_method(None, true).unwrap(), Method::POST);
        assert_eq!(pick_method(Some("patch"), true).unwrap(), Method::PATCH);
    }

    #[test]
    fn parse_headers_roundtrip() {
        let h = parse_headers(&["X-A: 1".into(), "X-B:two".into()]).unwrap();
        assert_eq!(h, vec![("X-A".into(), "1".into()), ("X-B".into(), "two".into())]);
    }

    #[test]
    fn build_body_simple() {
        let b = build_body(&["title=Bug".into(), "body=Details".into()], &[])
            .unwrap()
            .unwrap();
        assert_eq!(b["title"], "Bug");
        assert_eq!(b["body"], "Details");
    }

    #[test]
    fn build_body_empty_returns_none() {
        assert!(build_body(&[], &[]).unwrap().is_none());
    }
}
