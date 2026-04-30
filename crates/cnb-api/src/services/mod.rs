//! Service facades. M2 adds repos / issues / labels / pulls / uploads on top
//! of M1's `users`. M3 adds builds / workspaces / releases. M4 adds
//! registries / missions / orgs / repo_extras.

pub mod builds;
pub mod issues;
pub mod labels;
pub mod missions;
pub mod orgs;
pub mod pulls;
pub mod registries;
pub mod releases;
pub mod repo_extras;
pub mod repos;
pub mod uploads;
pub mod users;
pub mod workspaces;
