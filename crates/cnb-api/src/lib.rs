//! HTTP core + service facades for cnb CLI.
//!
//! Public surface in M1 is intentionally tiny:
//!
//! - [`Client`] — single-flight HTTP client wrapping `reqwest`.
//! - [`ApiError`] — unified error model.
//! - [`services::users::get_self`] — `GET /user` (used by `cnb auth login` to validate a token).
//! - [`Client::request_value`] — generic JSON passthrough used by `cnb api`.
//!
//! M2 will introduce per-resource service facades (Repos/Issues/Pulls/...) and
//! a `generated/` module produced by `progenitor`.

pub mod client;
pub mod error;
pub mod retry;
pub mod services;
pub mod tracing_layer;
pub mod url_safe;

pub use client::{Client, ClientBuilder, PassthroughResponse, DEFAULT_BASE_URL};
pub use error::{ApiError, ApiResponseError};
