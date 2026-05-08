//! Service facades.
//!
//! After Phase 2 of the cnb-api → typed SDK migration (see
//! [`cnb-cli`'s migration narrative in `docs/sdk-issues.md`]), this
//! crate keeps only two facades:
//!
//! - [`users::get_self`] — `GET /user`. Used by `cnb auth login` to
//!   validate a freshly-pasted token *before* it is persisted, where
//!   constructing a typed SDK client would force the full token-
//!   resolution dance (env > keyring > file) before the token even
//!   exists.
//! - [`uploads`] — two-phase asset upload for `cnb issue create
//!   --attach` and `cnb issue comment --attach`. The SDK's HTTP layer
//!   is JSON-only and cannot stream raw bytes (SDK-I14), so we keep
//!   this facade until the SDK exposes the corresponding helper or
//!   until the CLI's `--attach` path is rewritten on top of a side-
//!   car `reqwest::Client` directly.
//!
//! Every other facade (issues / pulls / repos / labels / builds /
//! workspaces / releases / registries / missions / orgs / repo_extras)
//! has been removed — `cnb-cli` calls into `cnb_sdk::*` instead.

pub mod uploads;
pub mod users;
