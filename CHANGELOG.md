# Changelog

All notable changes are documented here. The format roughly follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once
v1.0 is reached. Pre-1.0 releases may break in any minor bump.

## [Unreleased]

### Added (cnb-api → typed SDK migration, Phase 2 step 2.9)

- **`cnb mission` — all 6 subcommands ported to the typed SDK**
  (`cnb_sdk::missions::MissionsClient`). Covers `delete` /
  `view-list` / `view-edit` / `view-sort` / `view-get` / `view-set`.
  `view-edit` and `view-set` now parse the user-supplied JSON
  config file into `MissionView` / `MissionViewConfig` before
  handing it to the SDK — malformed payloads surface as a CLI
  `BadArgs` (exit 3) instead of getting round-tripped to the server.
- **`cnb registry` — all 10 verbs ported to the typed SDK**
  (`cnb_sdk::registries::RegistriesClient`). Covers `list` /
  `delete` / `set-visibility` / `package {list,view,delete}` /
  `tag {list,view,delete,provenance}`. The shape of the
  `set-visibility` request changes from a JSON body
  `{visibility_level: 0|10|20}` to the query string
  `?visibility=public|internal|private` — same story as
  `repo set-visibility` (SDK-I12). The integer-level translation is
  deleted from the CLI; forward the string verbatim.
  - `registry tag list` uses the raw HTTP path via
    `Context::sdk_raw_get` because the SDK's typed
    `list_package_tags` returns a single-object `models::Tag`
    DTO (the git-tag shape), which cannot deserialise the array
    the server actually emits. New SDK issue SDK-I15.
- **`cnb org` — all 7 subcommands ported** across three SDK
  resource clients (`organizations`, `members`, `followers`) plus
  `users::get_user_info` for the `--user` fallback. Covers
  `list` / `view` / `member {list,add,remove,edit}` /
  `follower` / `following`.
  - `org list` now reads the slug from `OrganizationAccess.path`
    rather than a fictitious `slug` field.
  - `org member add/edit` now sends the typed
    `UpdateMembersRequest { access_level, is_outside_collaborator }`
    body. The previous cnb-api facade sent `{username, role}` /
    `{role}` — a divergent wire shape. New SDK issue SDK-I16.
  - `org member list` commits to the typed `access_level` field;
    legacy `role` key tolerance on the response side is deliberately
    dropped (matches every other typed-first port in Phase 2).
  - `org follower` / `following` (no explicit user) now probe
    `users().get_user_info()` → `UsersResult.username` for the
    current user. Stays independent of the `auth login` flow which
    still runs `cnb_api::services::users::get_self` during token
    validation (Phase 1 residue).

### Milestone marker

- **Phase 2 functionally complete** for every command the SDK
  covers: `repo` (10/14), `issue`, `pr`, `label`, `release`,
  `build`, `workspace`, `registry`, `mission`, `org`, `search`.
  The cnb-api facade is now only used by:
  - `cnb auth login/logout/status` (Phase 1 token validation via
    `users::get_self`).
  - `cnb api` raw passthrough (structurally cannot use the SDK's
    JSON-only transport — see SDK-I14).
  - `cnb repo pin/unpin/list-pinned/contributors` (SDK does not
    expose those endpoints yet).

### Test additions

17 new wiremock integration tests in `m4_registry_mission_org`:

- Registry: list / set-visibility (query-string path) /
  set-visibility (invalid value rejection) / package list with
  `--type` filter / package view JSON round-trip through the
  typed `CommonRegistryPackageDetail` / tag list via the
  raw-passthrough workaround.
- Mission: delete (`--yes`) / view-sort posting `{ids:[…]}` /
  view-edit rejecting malformed JSON before any HTTP call /
  view-list emitting the typed array.
- Org: list reading slug from `path` / view card /
  member add posting `access_level` / member edit putting
  `access_level` / member list rendering typed `access_level` /
  follower fallback to `/user` then `/users/{me}/followers` /
  following with explicit `--user`.

Total project test count: 217 → **233** (the test suite from
step 2.8 grew 210→217; step 2.9 adds 17 more, for 233 overall).

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I15** (new, Tier A): `list_package_tags` returns a
  single-object git-tag DTO where the server emits an array.
  Typed path is unusable — downstream has to bypass the SDK to
  render tag lists. `cnb registry tag list` uses `sdk_raw_get`
  as the workaround.
- **SDK-I16** (new, Tier C): `UpdateMembersRequest` body shape
  (`access_level` / `is_outside_collaborator`) diverges from the
  prior cnb-api facade shape (`username` / `role`). Going with the
  SDK on the assumption it tracks the OpenAPI spec. Integration
  evidence against a real cnb.cool server is pending.

### Added (cnb-api → typed SDK migration, Phase 2 step 2.8)

- **`cnb build` — all 8 subcommands ported to the typed SDK**
  (`cnb_sdk::build::BuildClient`). Covers `run` / `list` / `status`
  (incl. `--watch` polling) / `view` (stage) / `logs` / `cancel` /
  `delete-logs` / `crontab-sync`. Typed DTOs (`StartBuildReq`,
  `BuildResult`, `LogInfo`, `BuildStatusResult`, `BuildStageResult`,
  `BuildCommonResult`) replace the hand-written cnb-api bodies.
- **`cnb workspace` (alias `ws`) — all 5 subcommands ported to the
  typed SDK** (`cnb_sdk::workspace::WorkspaceClient`). Covers
  `list` / `start` / `view` / `stop` / `delete`. The `view`
  card-style output now drives off `WorkspaceDetailResult`; the
  key loop still reads via `Value::get` because the seven access-
  channel names stay on the wire shape (`webide`, `remoteSsh`,
  `jumpUrl`, …).
- `cnb build logs` (runner log download) uses the same side-car
  `reqwest::Client` pattern as `release download` — SDK's typed
  `build_runner_download_log` returns `serde_json::Value` but the
  server emits plain text. Third bytes endpoint to hit this issue;
  see SDK-I14 (now covers `release upload` phase 2 + `release
  download` + `build logs`).

### Milestone marker

- **Phase 2 milestone**: with `build` and `workspace` done, every
  M3 top-level command (`release`, `build`, `workspace`) is on
  the SDK. Combined with M2 (`repo`, `issue`, `pr`, `label`) and
  M4-adjacent `search`, **the active front-line command surface
  is now SDK-backed**. Remaining consumers of the `cnb-api` facade:
  M4 groups (`registry`, `mission`, `org`), `repo pin/contributors`,
  and the `api` raw passthrough. Those are the Step 2.9+ scope.

### Test additions

- 7 new wiremock integration tests in `m3_build_workspace`:
  - `build_logs_downloads_plain_text_body` — end-to-end coverage of
    the new side-car bytes path (GET → stdout → plain-text body).
  - `build_logs_rejects_slashed_pipeline_id` — CLI-level guard
    (`BadArgs` exit 3) mirrors the `ensure_no_slash` check we used
    to get from the cnb-api layer, now moved into the command.
  - `build_list_reads_typed_fields` — asserts the `LogInfo` typed
    DTO carries `sn` / `status` / `sourceRef` / `targetRef` /
    `createTime` end-to-end to TSV output.
  - `build_cancel_with_yes_succeeds` — happy-path POST to
    `/{repo}/-/build/stop/{sn}` via `stop_build`.
  - `build_crontab_sync_hits_post_endpoint` — confirms
    `build_crontab_sync` routes to the right path.
  - `workspace_delete_with_yes_and_pipeline_id` — typed
    `WorkspaceDeleteReq { pipelineId }` body on `POST /workspace/delete`.
  - `workspace_view_card_lists_channels` — typed
    `WorkspaceDetailResult` → `Value` → key-lookup render across
    the four most common access channels.

  Total project test count: 210 → **217**.

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I14** extended to cover `build_runner_download_log` — same
  root cause (JSON-only transport), different endpoint. Severity
  stays *annoyance* but the case for a generic fix is now stronger
  because three independent verbs share the same workaround.

### Added (cnb-api → typed SDK migration, Phase 2 step 2.7)

- **`cnb release` — all 9 subcommands ported to the typed SDK**
  (`cnb_sdk::releases::ReleasesClient`). Covers `list` / `view`
  (by tag / `--id` / `--latest`) / `create` / `edit` / `delete` /
  `upload` / `download` / `asset-view` / `asset-delete`.
- The **two-phase asset upload** now mixes the SDK and a small
  side-car `reqwest::Client`:
  - Phase 1 (`POST asset-upload-url`) uses the typed SDK
    (`post_release_asset_upload_url` → `ReleaseAssetUploadUrl`).
  - Phase 2 (`PUT upload_url` with streamed file bytes) bypasses
    the SDK — the SDK's shared HTTP layer is JSON-only and cannot
    express a raw-bytes body. Uses `reqwest::Body::wrap_stream` +
    `tokio_util::io::ReaderStream` directly. See SDK-I14.
  - Phase 3 (`POST verify_url`) reuses the SDK's
    `HttpInner::execute(method, url)` — the verify URL is absolute
    but the SDK accepts that, and the response is JSON.
- `cnb release download` also uses the standalone
  `reqwest::Client` path: the SDK's `get_releases_asset` decodes
  the response as `serde_json::Value` which is wrong for a bytes
  endpoint. Base-URL precedence mirrors the SDK
  (`CNB_API_BASE` > default) so wiremock fixtures keep working.

### Test additions

- 2 new wiremock integration tests in `m3_release`:
  - `release_download_writes_bytes_to_output_dir` — covers the
    new raw-bytes download path end-to-end (GET → write to
    `--output` dir → on-disk content assertion).
  - `release_asset_view_emits_json_when_flag_set` — validates
    that the typed `get_release_asset` → `ReleaseAsset` →
    `serde_json::Value` render chain preserves every rendered
    field under `--json`.
  - Existing `release_upload_runs_two_phase_chain` now covers
    the SDK + side-car hybrid flow. Total project test count:
    208 → 210.

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I14** (new): the SDK's `HttpInner` is JSON-only on both
  request and response sides. Two legitimate flows — two-phase
  upload phase 2 (raw PUT) and release asset download (bytes
  body) — cannot use the SDK at all. Consumers must build a side-
  car `reqwest::Client`, which also means they cannot reuse the
  SDK's connection pool or resolved token (no public
  `reqwest_client()` accessor on `HttpInner`).

### Changed

- `cnb-cli` now depends on `tokio-util` (feature `io`) for the
  `ReaderStream` used by the release-upload phase 2 PUT. Was
  previously only pulled in transitively via `cnb-api`.

### Added (cnb-api → typed SDK migration, Phase 2 step 2.6)

- `cnb repo` write paths — `create`, `edit`, `delete`, `archive`,
  `unarchive`, `transfer`, `set-visibility`, `fork` (list forks) —
  now all route through `cnb_sdk::repositories::RepositoriesClient`.
  Combined with the read paths from step 2.1, **the entire
  first-party `cnb repo` surface is on the SDK** (10/14 verbs).
  Pin / unpin / list-pinned / contributors stay on the cnb-api
  `repo_extras` facade until the SDK exposes those endpoints.

### Changed — surface gaps now made explicit

- `cnb repo create --default-branch` is rejected with `BadArgs`
  (exit 3). The SDK's `CreateRepoReq` does not include a default-
  branch field; the cnb-api facade silently dropped it before. We
  surface the gap rather than pretending. See SDK-I11.
- `cnb repo edit --name <RENAME>` and `--default-branch` are
  similarly rejected. The SDK's `RepoPatch` body only carries
  `description / license / site / topics`. Only `--description`
  is currently honoured by the PATCH `/{repo}` endpoint.
- `cnb repo set-visibility` now sends `?visibility=…` as a **query
  parameter** (the SDK shape, tracking the OpenAPI spec) instead of
  a `{visibility_level: 0|10|20}` body. If a real cnb.cool server
  rejects the new shape, the issue is logged as SDK-I12 and we'll
  fall back via raw HTTP.
- `cnb repo fork` unwraps the SDK's `ListForks { fork_tree_count,
  forks: Option<Vec<Forks>> }` envelope so `--json` output stays a
  bare array, matching `gh repo fork`'s shape and the previous
  cnb-api facade's behaviour. See SDK-I13.

### Test additions

8 new wiremock integration tests in `m2_repo` covering:
`repo create` (body shape + `--default-branch` rejection),
`repo edit --description` (single-field PATCH + `--name`
rejection), `repo archive` / `unarchive`, `repo transfer --yes`
(verifies `target` in body, `source` omitted), `repo set-visibility`
(verifies query-string shape, NOT body), `repo fork` (verifies the
`ListForks` envelope is unwrapped to a plain array). Total project
test count: 200 → 208.

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I11** (new): `RepoPatch` is a strict subset of what `cnb
  repo edit` historically accepted — no `name`, no
  `default_branch`. Surfaces a real gap the cnb-api facade was
  masking.
- **SDK-I12** (new): `set_repo_visibility` uses a query string
  rather than a JSON body. Disagrees with the prior cnb-api facade.
  Following the SDK pending wire confirmation.
- **SDK-I13** (new): `list_forks_repos` returns a wrapper struct,
  not a `Vec`. Inconsistent with every other `list_*` method in
  the SDK.

### Removed

- `commands::repo::visibility_to_level()` and its associated unit
  test. SDK aliases `Visibility = String`, so we forward
  `public|internal|private` verbatim and never need the integer
  encoding on the request side. (`format_visibility` on the
  *display* side stays — it still tolerates legacy integer
  responses from older servers.)

### Added (cnb-api → typed SDK migration, Phase 2 step 2.5)

- **`cnb label` is now backed entirely by the typed SDK**
  (`cnb_sdk::repo_labels`). This is the **first command group ported
  in full**, including write paths (`create` / `edit` / `delete`).
  All four endpoints map cleanly: list → `GET /{repo}/-/labels`,
  create → `POST`, edit → `PATCH /{name}`, delete → `DELETE /{name}`.
- `ensure_label_name_safe()` guard mirrors the path-traversal check
  the cnb-api facade had (the SDK does not encode user-controlled
  path segments). Rejects `/` and empty names with `BadArgs` (exit
  3) before any HTTP round-trip. 4 unit tests + 1 integration test
  for `label delete evil/../leak`.
- 5 new wiremock integration tests covering write paths:
  `edit --description`, `edit` with no fields (exit 3),
  `delete --yes`, `delete` without `--yes` in non-TTY (exit 3),
  `delete` with `/` in name (exit 3 before network).

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I10** (new): user-controlled identifiers interpolated into
  request paths (`format!("/{repo}/-/labels/{name}", …)`) are not
  validated or percent-encoded by the SDK. A label named `..` is
  silently routed to a different endpoint. Severity: annoyance —
  every consumer has to mirror the validation. Documented our
  workaround.

### Added (cnb-api → typed SDK migration, Phase 2 step 2.4)

- `cnb pr view` and `cnb pr list` (and their `cnb mr …` aliases) now
  route through the typed SDK (`cnb_sdk::pulls::PullsClient`). Like
  `cnb issue view`, a single typed call is enough for `view` — the
  `Pull` DTO already carries every field the CLI card renders.
- `format_pr_number()` and `read_branch()` helpers on
  `commands::pr`: the first mirrors `format_issue_number` (accepts
  string / integer / null); the second absorbs the upstream spec's
  wobble on PR head/base encoding (tries `branch`, `ref`, `name` on
  the typed object, then a legacy top-level sibling string). 7 new
  unit tests total.

### Changed

- `m2_label_pr` wiremock fixtures updated to the SDK DTO shape:
  `number` is now a string, and branch info lives inside nested
  `head: {branch}` / `base: {branch}` objects instead of top-level
  `source_branch` / `target_branch` strings. Real cnb.cool servers
  return the nested form; the CLI still accepts the legacy one via
  `read_branch` fallback.

### Tracked SDK friction (`docs/sdk-issues.md`)

- **SDK-I07** extended: the inconsistency is not just inside `issues`
  — it is also *across modules*. `get_issue` takes `i64` numbers and
  `get_pull` takes `String` numbers for analogous concepts. Both DTOs
  type the value as `Option<String>`. Logged.
- **SDK-I08** (new): the `pulls` resource ships two overlapping
  structs — `Pull` (returned by `get_pull`) and `PullRequest`
  (returned by `list_pulls`). Field sets overlap but do not match
  exactly (different `labels` types, `PullRequest`-only
  `comment_count` / `review_count` / `repo` / `created_at`,
  `Pull`-only `reviewers`). CLI serialises both through Value for
  uniform rendering.
- **SDK-I09** (new): `Pull.head` / `Pull.base` are untyped
  `Option<serde_json::Value>`. Every consumer has to reinvent a
  "branch-name extractor". Our `read_branch` helper covers the three
  observed shapes + the legacy top-level fallback.

### Added (cnb-api → typed SDK migration, Phase 2 step 2.3)

- `cnb issue view` and `cnb issue list` (both the repo-scoped path and the
  `--mine` / `/user/issues` variant) now go through the typed SDK
  (`cnb_sdk::issues::IssuesClient`). The typed `IssueDetail` DTO is rich
  enough that `view` uses a single typed call — no raw-Value double-fetch
  is needed (unlike `repo view` — see SDK-I02).
- `format_issue_number()` helper on `commands::issue`: tolerant display
  formatter mirroring `format_visibility`. Accepts both the spec's
  canonical string form and the legacy integer encoding that older
  cnb.cool deployments still emit. Unit-tested with 3 cases.
- `docs/sdk-issues.md`: running log of SDK-side friction points
  discovered during Phase 2 (so far: `pub(crate)` on `HttpInner::url`,
  missing `default_branch` in `Repos4User`, `Visibility` string / int
  ambiguity, crate-name collision, missing `#[non_exhaustive]`,
  unreachable upstream repo URL, and the freshly minted SDK-I07 about
  `get_issue(number: i64)` vs `Issue.number: Option<String>`).

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
