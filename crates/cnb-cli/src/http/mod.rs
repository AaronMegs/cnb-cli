//! Thin HTTP utilities that ride on top of the typed `cnb-sdk` client.
//!
//! After the cnb-api → cnb-sdk migration completed (Phase 2 + the
//! `users::get_self` follow-up + the `release/build bytes` and `pin`
//! cleanups landed against cnb 0.2.2), the only HTTP shapes that the
//! typed SDK does **not** model are:
//!
//! - `cnb api …` — generic gh-style passthrough where callers want raw
//!   `(status, headers, body_text)` rather than a deserialised JSON DTO.
//! - `cnb issue create --attach` / `cnb issue comment --attach` — two-
//!   phase multipart uploads streamed from disk.
//!
//! Both are implemented here in terms of the SDK's shared
//! `reqwest::Client` (so we keep its connection pool, default
//! `Authorization: Bearer …` header, and base-URL precedence) plus
//! `HttpInner::url(path)` (so we keep the SDK's URL construction —
//! percent-encoding, trailing-slash normalisation, query passthrough).
//!
//! The previous `cnb-api` crate that hosted these has been retired.

pub mod passthrough;
pub mod sensitive;
pub mod uploads;
