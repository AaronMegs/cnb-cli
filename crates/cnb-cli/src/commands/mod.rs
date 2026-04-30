//! Command groups. M1 ships `auth` and `api`; M2 adds `repo`/`issue`/`label`/`pr`;
//! M3 adds `build`/`workspace`/`release`; M4 adds
//! `registry`/`mission`/`org`/`browse`/`completion`/`config`/`alias`.

pub mod alias;
pub mod api;
pub mod auth;
pub mod browse;
pub mod build;
pub mod completion;
pub mod config;
pub mod issue;
pub mod label;
pub mod mission;
pub mod org;
pub mod pr;
pub mod registry;
pub mod release;
pub mod repo;
pub mod update;
pub mod workspace;
