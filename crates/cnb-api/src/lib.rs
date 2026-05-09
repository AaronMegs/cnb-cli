//! HTTP core + a narrow service facade for `cnb` CLI.
//!
//! After Phase 2 of the cnb-api → typed SDK migration plus the
//! follow-up `users::get_self` removal, this crate only ships:
//!
//! - [`Client`] — single-flight HTTP client wrapping `reqwest`, still
//!   used by `cnb api` raw passthrough.
//! - [`ApiError`] — unified error model (shared between the remaining
//!   passthrough paths).
//! - [`services::uploads`] — two-phase asset upload used by
//!   `cnb issue create --attach` / `cnb issue comment --attach`
//!   (blocked on SDK-I14; will be removed once the SDK grows a
//!   non-JSON transport surface).
//! - [`Client::request_value`] — generic JSON passthrough used by
//!   `cnb api`.
//!
//! All other verbs route through the typed SDK (`cnb-sdk`, the
//! external `cnb` crate). See `docs/sdk-issues.md` for the
//! migration narrative.

pub mod client;
pub mod error;
pub mod retry;
pub mod services;
pub mod tracing_layer;
pub mod url_safe;

pub use client::{Client, ClientBuilder, PassthroughResponse, DEFAULT_BASE_URL};
pub use error::{ApiError, ApiResponseError};
