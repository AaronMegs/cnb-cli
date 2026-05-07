# Changelog

All notable changes are documented here. The format roughly follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once
v1.0 is reached. Pre-1.0 releases may break in any minor bump.

## [Unreleased]

### Added (cnb-api → typed SDK migration, Phase 2 step 1)

- `cnb repo view` and `cnb repo list` now route through the typed SDK
  (`cnb_sdk::repositories::RepositoriesClient`). Dispatch mirrors the
  cnb-api facade: no `target` → `GET /user/repos`, slug with `/` → `GET
  /{slug}/-/repos`, bare username → `GET /users/{u}/repos`.
- `Context::sdk_raw_get(path)`: shared helper for SDK-backed commands
  that still need faithful `Value` passthrough on `--json` / `--jq` /
  `--template` (e.g. `repo view`'s `default_branch` field is not in the
  typed DTO). Routes through the SDK's own reqwest pool so retry / auth
  / tracing semantics match the typed calls byte-for-byte.
- `Context::effective_sdk_base_url()` (internal): single source of truth
  for base-URL resolution — explicit override > `CNB_API_BASE` > SDK
  default. Removes the last bit of duplicated logic between `sdk()` and
  the new raw helper.
- `format_visibility()` on `commands::repo`: tolerant display formatter
  that accepts both the spec's canonical string form
  (`"public"`/`"internal"`/`"private"`) and the legacy integer encoding
  (`0`/`10`/`20`) some older servers still emit. Unit-tested.

### Changed

- The SDK-typed `Repos4User` DTO aliases `visibility_level` to
  `Visibility = String`, so all `cnb repo view/list` wiremock fixtures
  now model it as a string. The previous integer encoding used by
  hand-written mocks was a bug that only worked because the old facade
  re-parsed raw `Value` and made its own guesses.
- `view` endpoint is fetched twice on purpose for SDK-backed single-
  object views: once typed (catches schema regressions) and once as raw
  `Value` (faithful rendering). Both hit the same reqwest connection
  pool — extra cost is one round-trip on a single object, negligible.

### Added (cnb-api → typed SDK migration, Phase 1)

- Depend on external crate `cnb = "0.2"` (aliased as `cnb-sdk` in workspace
  manifests to avoid collision with the local `cnb` binary). Covers all
  241 CNB OpenAPI operations across 28 tag groups, published by the same
  maintainer and generated from the same swagger spec.
- New top-level command `cnb search` backed by `cnb_sdk::search` — the
  first consumer of the typed SDK in this repo. Single endpoint
  (`GET /search/public-repos`), pure read, read-only DTO round trip; chosen
  as a low-risk pilot for the wider migration (Phase 2 will port the rest
  of `crates/cnb-api/src/services/*.rs` module by module).
- `Context::sdk()` on the CLI runtime context: three-tier token resolver
  (`env > keyring > file`, identical to `Context::api()`) feeding
  `cnb_sdk::ApiClient::builder().token(...)`. Honours `CNB_API_BASE` for
  wiremock test parity with every existing integration fixture.
- `Context::set_sdk_base_url()`: test-only hook for unit tests that need to
  build the client against an arbitrary URL without going through the env.
- `CliError::Sdk(cnb_sdk::ApiError)` variant with HTTP-status → exit-code
  mapping (401 → 4, 404 → 2, 429 → 8, 5xx → 9) mirroring the existing
  `Api` arms.
- 5 wiremock integration tests (`crates/cnb/tests/search_sdk.rs`) covering
  table output, query-parameter forwarding, `--json`, `--jq`, and the
  401→exit 4 error path.

### Added (M5.1 distribution prep)

- `cargo xtask gen-man` — render man pages (one per leaf command, 128 files
  for the current command tree) from clap definitions.
- `cargo xtask gen-completions` — render bash / zsh / fish / powershell /
  elvish completions from clap definitions.
- `cargo xtask gen-dist` — convenience wrapper that runs both of the above
  into `dist/`.
- `release.yml` now ships `man/` and `completions/` inside every per-target
  archive and applies cosign keyless signatures (`*.sig` + `*.pem`) to each
  archive using GitHub Actions OIDC + Sigstore Fulcio (no secrets required).
- mdbook handbook scaffold under `docs/` with `docs.yml` workflow.
- Distribution-channel templates under `dist-templates/`:
  - `homebrew/cnb.rb.tmpl` (Homebrew tap formula)
  - `scoop/cnb.json.tmpl` (Scoop bucket manifest)

### Changed

- `deny.toml`: advisories now `yanked = "deny"` (was "warn") so cargo-deny
  fails the build on a known-bad transitive dep.
- `repo set-visibility` argument order: visibility is now the leading
  positional and `--repo OWNER/REPO` is a flag (was: trailing positional
  `repo` followed by required `visibility` — an invalid clap layout that
  surfaced as a `_verify_positionals` panic when introspecting the command
  tree).

### Fixed

- xtask now compiles cleanly when invoked through `cargo run`; the `Cli`
  type from `cnb-cli` is reused so man pages and completions stay in lock-
  step with the actual CLI surface.

## [0.4.0-alpha.1] — 2026-04-30

First feature-complete preview covering all 14 command groups (M1 → M4).
17 top-level commands, 115+ subcommands, 173 tests passing.

### Added (M4 — peripheral capabilities)

- `cnb registry` — 11 typed package families (docker / helm / maven / npm /
  pypi / rubygems / composer / nuget / golang / conan / generic) with
  list/view/stats + packages list/view/delete + tags/rules/hooks.
- `cnb mission` — list/view/create/edit/delete/run.
- `cnb org`, `cnb member` — org/group/member management (7 verbs).
- `cnb repo collaborator|pin|activity|contributors` — augments repo with
  the four collaboration verbs.
- `cnb browse` — open the current resource in the default browser.
- `cnb completion` — emit shell completions for 5 shells.
- `cnb config` / `cnb alias` — user preferences and aliases.
- `cnb auth setup-git` — register a git credential helper that shells out
  to `cnb auth token` (no plaintext token in `~/.gitconfig`).

### Added (M3 — platform specialty)

- `cnb build` — run/list/status/view/logs/watch/cancel/delete-logs/
  crontab-sync (8 verbs, with indicatif spinner + ctrl-c on `watch`).
- `cnb workspace` (alias `ws`) — list/start/view/stop/delete (5 verbs).
- `cnb release` — list/view/create/edit/delete/upload/download/asset-view/
  asset-delete (9 verbs); upload uses two-stage pre-signed PUT + verify
  POST and streams via tokio-util ReaderStream.
- `cnb pr review|checks|batch`, `cnb issue activity|properties` — completes
  PR/issue coverage.

### Added (M2 — repo/issue/label/pr)

- `cnb repo` — 11 verbs (list/view/create/clone/fork/edit/archive/
  unarchive/transfer/set-visibility/delete).
- `cnb issue` — 11 verbs.
- `cnb label` — 4 verbs.
- `cnb pr` (alias `mr`) — 12 verbs incl. checkout/diff/commits.
- File / image attachment via `--attach`.

### Added (M1 — skeleton & core, 0.1.0)

- 8-crate workspace: `cnb / cnb-cli / cnb-api / cnb-config / cnb-auth /
  cnb-git / cnb-tty / xtask`.
- HTTP core: single reqwest client, auth middleware, retry, ApiError, redact.
- Auth: env > keyring > file three-tier resolver; login/logout/status/token.
- Config: `config.toml` / `hosts.toml` schema v1, atomic writes + file lock.
- Generic `cnb api` (GET/POST/PATCH/PUT/DELETE, `-f/-F/-H/--paginate/--jq/
  --template`).
- Output: TTY detection, comfy-table, JSON/jq/template, NO_COLOR.
- 3-OS × stable/MSRV CI matrix.

[Unreleased]: https://cnb.cool/cnb/cli/-/compare/v0.4.0-alpha.1...HEAD
[0.4.0-alpha.1]: https://cnb.cool/cnb/cli/-/releases/v0.4.0-alpha.1
