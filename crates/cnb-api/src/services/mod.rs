//! Service facades.
//!
//! After Phase 2 of the cnb-api → typed SDK migration plus the
//! follow-up in the `users::get_self` removal commit (see `docs/sdk-issues.md`),
//! this crate keeps **one** facade:
//!
//! - [`uploads`] — two-phase asset upload for `cnb issue create
//!   --attach` and `cnb issue comment --attach`. The SDK's HTTP layer
//!   is JSON-only and cannot stream raw bytes (SDK-I14), so we keep
//!   this facade until the SDK exposes the corresponding helper or
//!   until the CLI's `--attach` path is rewritten on top of a side-
//!   car `reqwest::Client` directly.
//!
//! Every other facade (issues / pulls / repos / labels / builds /
//! workspaces / releases / registries / missions / orgs / repo_extras /
//! users) has been removed — `cnb-cli` calls into `cnb_sdk::*` instead.

pub mod uploads;
