# SDK issues log

Running tally of friction points encountered while porting `cnb-cli`
commands to the external `cnb` crate (aliased as `cnb-sdk` in our
workspace manifests, published by AaronMegs on crates.io).

**Goal of this doc**: batch-report a clean list of actionable issues to
the SDK upstream once Phase 2 completes, instead of peppering the
maintainer with one-off tickets per port.

Each entry has:

- **Id** — short stable slug for cross-referencing in commits.
- **Surface area** — which SDK file / API is affected.
- **Summary** — what went wrong or what is missing.
- **Workaround** — what we did in this repo to keep moving.
- **Desired fix** — what we'd like to see upstream.
- **Severity** — blocker / annoyance / polish.

---

## Triage summary (2026-05-08, Phase 2 steps 2.1–2.9 complete)

Counts: **16 open, 0 resolved**. Grouped below by how we plan to
surface them upstream — not by the individual severity tags inside
each entry, which stay as the original per-port verdict.

### Tier A · file as standalone issues before Phase 2 ends

These are the cases where every single consumer has to reinvent the
same workaround *and* where the workaround loses a real SDK feature
(typed access, connection-pool reuse, auth forwarding). They deserve
their own upstream issues with a minimal repro, not a summary bullet.

| Id       | One-line pitch                                                                 | Why tier A                                                                      |
|----------|--------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| SDK-I03  | `Visibility` alias does not accept integer-form responses                      | Blocks typed deserialisation on any server still emitting `0/10/20`.            |
| SDK-I09  | `Pull.{head,base}` typed as `Option<Value>` — every UI reinvents `read_branch` | ~50% of MR tooling code is branch-name extraction; the SDK could own it.       |
| SDK-I14  | No non-JSON transport — bytes endpoints need a side-car `reqwest::Client`      | Forces two HTTP clients per `release upload` / `release download` / `build logs`; duplicates auth. |
| SDK-I07  | Issue vs pull number typing disagrees across methods (`i64` vs `String`)       | Contagious — every downstream type definition has to pick a side and stick.    |
| SDK-I15  | `list_package_tags` returns single-object `Tag` instead of `Vec<TagSummary>`   | Typed path is unusable — registry UIs must bypass the SDK to render tag lists. |

### Tier B · file together as a "generated-DTO polish" bundle

Small, related DTO / method-signature nits that a single upstream
patch can clean up. None block compilation, but together they cost the
consumer non-trivial boilerplate.

| Id       | One-line pitch                                                                  | Proposed grouping         |
|----------|---------------------------------------------------------------------------------|---------------------------|
| SDK-I02  | `Repos4User` DTO omits `default_branch` (and likely other server-side fields)   | DTO completeness          |
| SDK-I08  | Same resource, two DTOs (`Pull` vs `PullRequest`) — share rendering is painful  | DTO completeness          |
| SDK-I11  | `RepoPatch` cannot express `name` / `default_branch` — prior facade was lying   | DTO completeness          |
| SDK-I13  | `list_forks_repos` returns `ListForks { forks: Option<Vec<_>> }` instead of vec | Signature consistency     |
| SDK-I01  | `HttpInner::url()` + `base_url` are `pub(crate)` — no clean escape hatch        | Method visibility         |

### Tier C · batch into a single "polish / conventions" ticket

Housekeeping. Each one alone doesn't warrant a ticket, but one
well-written meta-issue covering them lets the maintainer triage as a
group.

| Id       | One-line pitch                                                                       | Subgroup                 |
|----------|--------------------------------------------------------------------------------------|--------------------------|
| SDK-I04  | Crate name `cnb` collides with any binary also named `cnb`                           | Publishing metadata       |
| SDK-I05  | Query structs lack `#[non_exhaustive]`; direct-init becomes a SemVer hazard          | Generated-code conventions |
| SDK-I06  | `repository` URL in crate metadata 404s on unauthenticated visit                     | Publishing metadata       |
| SDK-I10  | No path-segment validation for user-controlled identifiers                           | Defensive defaults        |
| SDK-I12  | `set_repo_visibility` uses a query string, not a body — unclear which is canonical   | Spec / server alignment   |
| SDK-I16  | `UpdateMembersRequest` body shape disagrees with prior facade; spec vs wire unclear  | Spec / server alignment   |

### Upstream-report rollout plan

1. Land Phase 2 fully (remaining: `cnb api` raw passthrough — may
   never be portable, and `repo pin`/`contributors` which the SDK
   does not yet expose).
2. File the **five Tier A issues** first — one each, with a minimal
   reproducible code sample pulled straight from `cnb-cli` git
   history. They stand on their own and have the clearest upstream ask.
3. File the **Tier B bundle** as one consolidated issue titled
   something like *"DTO completeness & method-signature consistency
   during the cnb-cli port"*, linking to specific commits per sub-case.
4. File the **Tier C meta-issue** last — list the six sub-items
   with a one-paragraph justification each; no reproducer required.
5. Offer a patch PR for whatever looks least controversial (I04
   rename, I05 `#[non_exhaustive]`, I06 URL fix are all safe
   starting points).

---

## Active issues (v0.2.1, 2026-05-07)

### SDK-I01 · `HttpInner::url()` and `base_url` are `pub(crate)`

- **Surface**: `cnb::http::HttpInner`
- **Summary**: `HttpInner::url(path)` and the `base_url: Arc<Url>` field
  are `pub(crate)`, so downstream callers cannot reach the
  already-resolved base URL to construct ad-hoc requests through the
  SDK's own HTTP layer. `HttpInner::execute::<T>` is `pub` and accepts
  an arbitrary `Url`, but nobody outside the crate can build that `Url`
  correctly (base-URL precedence, trailing-slash normalisation, percent-
  encoding) without duplicating the SDK's logic.
- **Workaround**: `cnb-cli::Context::sdk_raw_get(path)` recomputes the
  base URL from its own sources (`CNB_API_BASE` > SDK default) and uses
  `url::Url::parse().join()` to build the final URL, then hands it to
  `HttpInner::execute::<Value>(GET, url)`. Works, but it means the CLI
  has to know two separate things the SDK already knows: the base URL,
  and the "trailing slash" rule for `Url::join`.
- **Desired fix**: Expose either:
  1. `pub fn HttpInner::url(path: &str) -> Result<Url>`  (make the
     existing method public), or
  2. `pub fn ApiClient::base_url() -> &Url` + a documented
     `url::Url::join()` contract, or
  3. A higher-level convenience:
     `HttpInner::get_json<T>(path: &str) -> Result<T>` that wraps
     `url()` + `execute::<T>()`.
- **Severity**: annoyance — blocks clean raw-Value escape hatches,
  which are necessary whenever the typed DTO omits fields the server
  actually returns (see SDK-I02).

### SDK-I02 · `Repos4User` DTO omits server-side fields

- **Surface**: `cnb::models::Repos4User`, returned by
  `repositories::get_by_id`
- **Summary**: The `Repos4User` struct does not model every field the
  real server returns for `GET /{repo}`. Concrete example: the
  production API returns `default_branch`, but the DTO has no such
  field, so a typed-only call would silently drop it; `--json` output
  would then lie to users.
- **Workaround**: `cnb repo view` fetches the endpoint twice — once
  typed (to catch schema regressions early) and once as raw
  `serde_json::Value` (to preserve every field for `--json` / `--jq` /
  `--template`). Both calls share the same reqwest connection pool so
  the extra cost is a single round-trip on a single-object GET.
- **Desired fix**: Add `default_branch` (and any other verified-present
  fields) to `Repos4User`. Longer term: generate the DTO directly from
  the server's OpenAPI spec with an option to keep a `#[serde(flatten)]
  extra: HashMap<String, Value>` catch-all so undocumented fields
  survive by default.
- **Severity**: annoyance — the workaround is cheap but duplicates the
  request; would become a real cost for list endpoints if we hit the
  same problem there.

### SDK-I03 · `Visibility` aliased to `String` but some servers emit int

- **Surface**: `cnb::models::Visibility = String`, used via
  `visibility_level: Option<Visibility>`
- **Summary**: The upstream OpenAPI spec models visibility as an enum
  of strings (`"public"` / `"internal"` / `"private"`), and the SDK
  honours that: `pub type Visibility = String;`. But older cnb.cool
  deployments and a fair chunk of our own wiremock fixtures still emit
  integers (0 / 10 / 20). A typed `Repos4User` deserialisation against
  an integer `visibility_level` fails with
  `invalid type: integer X, expected a string`.
- **Workaround**:
  1. Every wiremock fixture updated to the canonical string form.
  2. `cnb-cli::commands::repo::format_visibility()` tolerates both
     forms on the display path, so real servers that still emit
     integers don't look broken to end users (even though the typed
     deserialisation itself would have failed — this only helps for the
     raw `Value` passthrough used by `sdk_raw_get`).
- **Desired fix**: Make `Visibility` a proper enum with
  `#[serde(untagged)]` or a custom `Deserialize` that accepts both
  `"public"` and `0`, round-tripping to a canonical string on the way
  out. Alternatively: loosen the DTO to `Option<serde_json::Value>`
  until the server converges on one representation.
- **Severity**: blocker for anyone wiring the SDK against a cnb.cool
  instance that hasn't migrated to the string form yet.

### SDK-I04 · `cnb 0.2.x` package name collides with the binary

- **Surface**: workspace / downstream integration
- **Summary**: Our binary is also named `cnb`, so depending directly
  on `cnb = "0.2"` creates an ambiguous package name. Every workspace
  command that would otherwise accept `-p cnb` has to be disambiguated
  (`cargo test -p cnb@0.4.0-alpha.1` or the equivalent
  `--manifest-path` form).
- **Workaround**: `cnb-sdk = { package = "cnb", version = "0.2", ... }`
  in our workspace dep table, so the local name is `cnb-sdk` while
  cargo still pulls the `cnb` crate. All in-code references use
  `cnb_sdk::…`. Integration tests run via `--manifest-path
  crates/cnb/Cargo.toml`.
- **Desired fix**: Republish as `cnb-sdk` or `cnb-client` on crates.io
  so downstream bins named `cnb` can depend on it without the rename
  dance. The current name is problematic for any CLI or workspace
  whose bin target coincides — not an uncommon case for an API client
  crate.
- **Severity**: polish — the workaround is stable and documented, but
  the rename noise shows up in every Cargo.toml and every `use` line.

### SDK-I05 · No `#[non_exhaustive]` on query structs

- **Surface**: `GetReposQuery`, `GetReposByUserNameQuery`,
  `GetGroupSubReposQuery`, etc.
- **Summary**: Query parameter structs are plain `#[derive(Default)]`
  with public fields and no `#[non_exhaustive]`. If upstream adds a
  new optional query field in a future minor version, any consumer
  that built the struct with positional field init or pattern-matched
  on it would break. Builder methods exist (`q.page(n).page_size(m)`)
  and we use them, so we are fine today, but the escape hatch of
  direct struct init tempts users into SemVer hazards.
- **Workaround**: Prefer the builder chain
  (`GetReposQuery::new().page(…).page_size(…)`) in all CLI code; avoid
  `GetReposQuery { page: ..., ..Default::default() }` even though it
  compiles.
- **Desired fix**: Add `#[non_exhaustive]` to every query / body
  struct, or mark fields `pub(crate)` and force builder-only access.
- **Severity**: polish.

### SDK-I06 · README for the upstream repo is not reachable

- **Surface**: `https://cnb.cool/aodoo/tools/rust-cnb`
- **Summary**: The crate metadata points at
  `https://cnb.cool/aodoo/tools/rust-cnb.git` as the repository. The
  corresponding web URL returns 404 ("页面不存在或访问权限不足") on an
  unauthenticated visit. docs.rs content makes up for most of it, but
  anyone following the crates.io "Repository" link lands on a dead
  page.
- **Workaround**: read `docs.rs/cnb/0.2.1/cnb/` instead. The README
  that ships inside the `.crate` tarball has the useful bits.
- **Desired fix**: either make the repo public read or redirect the
  Cargo.toml `repository` field to a browsable mirror (GitHub /
  source.gc etc.).
- **Severity**: polish.

### SDK-I07 · Issue / Pull number type is inconsistent across the SDK surface

- **Surface**:
  - `cnb::issues::IssuesClient::get_issue(repo, number: i64)` vs
    `cnb::models::{Issue, IssueDetail, UserIssue}.number: Option<String>`
  - `cnb::pulls::PullsClient::get_pull(repo, number: String)` vs
    `cnb::models::{Pull, PullRequest}.number: Option<String>`
- **Summary**: The typed `get_issue` method takes `number: i64`, yet
  the DTO it returns (and every other issue DTO) types `number` as
  `Option<String>`. The same mismatch appears on the related endpoints
  (`list_issue_comments`, `get_issue_comment`, `update_issue`, etc —
  all take `i64` arguments but return string-valued `number`s). That
  forces every CLI consumer to convert between the two at the boundary
  and pushes an implicit assertion about what formats of issue number
  the server accepts.

  The `pulls` family is *internally* consistent (`get_pull(number:
  String)` + `Pull.number: Option<String>` — both strings), but
  *cross-module* it is inconsistent with `issues`: the same kind of
  "numeric resource id" is `i64` for issues and `String` for pulls.
  A CLI that threads both through the same types is forced to pick a
  favourite.
- **Workaround**:
  - `cnb-cli::commands::issue` passes `i64` to `get_issue`, converting
    from the CLI's `u64` parameter type through `i64::try_from`.
  - `cnb-cli::commands::pr` converts `args.number: u64` via
    `.to_string()` before calling `get_pull`.
  - Both CLI display layers use the same pattern
    (`format_issue_number` / `format_pr_number`) to tolerate string
    *and* integer encodings on the response side.
- **Desired fix**: pick one representation and apply it everywhere.
  If the spec really does want strings (to support sparse / scoped
  numbering), change the method arguments to `&str` / `String`.
  Otherwise, normalise the DTO to `Option<i64>`. Either way, make the
  choice uniform across `issues` and `pulls` — they model analogous
  concepts and should not disagree.
- **Severity**: annoyance — the conversion dance is contagious: any
  downstream code that threads issue numbers through its own layers
  has to pick one side and stick with it.

### SDK-I08 · Same resource, two different DTOs (`Pull` vs `PullRequest`)

- **Surface**: `cnb::pulls::PullsClient`
  - `get_pull()` returns `cnb::models::Pull`
  - `list_pulls()` returns `Vec<cnb::models::PullRequest>`
- **Summary**: Two generated structs for the same underlying resource,
  with overlapping but subtly different field sets:

  | field          | `Pull`                           | `PullRequest`                     |
  |----------------|----------------------------------|-----------------------------------|
  | `labels`       | `Vec<LabelInfo>`                 | `Vec<Label>`                      |
  | `comment_count`| absent                           | `Option<i64>`                     |
  | `review_count` | absent                           | `Option<i64>`                     |
  | `repo`         | absent                           | `Option<serde_json::Value>`       |
  | `created_at`   | absent                           | `Option<String>`                  |
  | `last_acted_at`| absent                           | `Option<String>`                  |
  | `reviewers`    | `Vec<PullReviewer>`              | absent                            |
  | `updated_at`   | `Option<String>`                 | `Option<String>`                  |

  This mirrors the upstream spec's distinction between "detail view" and
  "list item" — valid in theory, but painful in practice: a CLI that
  wants to share rendering code between `list` and `view` either has to
  pick the lowest common denominator (and lose typed access to a handful
  of fields) or deal with two parallel `match` arms.
- **Workaround**: Convert both DTOs to `serde_json::Value` at the
  rendering boundary in `commands::pr::{list, view}` and read every
  field through `Value::get()`. This loses the benefit of typed access
  on the hot path, but keeps a single render function.
- **Desired fix**: generate a single `Pull` struct with an optional
  `stats: Option<PullStats>` sub-struct (or similar) for the
  list-only bean counters. Alternatively, `#[serde(flatten)]` the
  shared portion into both so downstream code can destructure a
  common view. Cross-reference with the `Issue` / `IssueDetail`
  divergence, which has the same shape.
- **Severity**: annoyance.

### SDK-I09 · `head` / `base` fields are untyped `Option<Value>`

- **Surface**: `cnb::models::{Pull, PullRequest}.{head, base}: Option<serde_json::Value>`
- **Summary**: Pull request head/base branch information — one of the
  most-frequently-rendered fields in any MR UI — is typed as
  `Option<serde_json::Value>`. The upstream OpenAPI spec does not pin
  the schema, so the SDK correctly refuses to commit to a shape. But
  downstream consumers still have to extract a branch name to render
  anything useful, and the real server returns at least three
  different shapes across deployments:
  - `{head: {branch: "feat/x", commit_id: "abc"}}`
  - `{head: {ref: "refs/heads/feat/x"}}`
  - `{head: {name: "feat/x"}}`
  - Plus a legacy top-level sibling field `source_branch` / `target_branch`
    that the SDK drops entirely because it is not in the DTO.
- **Workaround**: `cnb-cli::commands::pr::read_branch()` tries each of
  `branch`, `ref`, `name` on the primary object in order, then falls
  back to a sibling top-level string (kept only for legacy
  deployments). Unit-tested with 5 cases covering every observed shape.
- **Desired fix**: settle the spec on one canonical encoding (ideally
  `{branch: "…", commit_id: "…"}` since that matches the server's
  current default), promote it to a real DTO (`PullRef` /
  `BranchSnapshot`), and type `head`/`base` as `Option<PullRef>`. This
  would turn a best-effort `read_branch` helper into a one-liner like
  `v.head.as_ref().and_then(|r| r.branch.as_deref()).unwrap_or_default()`.
- **Severity**: blocker-adjacent — not for compilation, but for any UI
  that wants branch info on a PR. Every consumer has to reinvent
  `read_branch`.

### SDK-I10 · No path-segment validation on user-controlled identifiers

- **Surface**: every resource client method that interpolates a
  user-controlled string into the request path. Concretely observed
  on:
  - `cnb::repo_labels::RepoLabelsClient::{delete_label, patch_label}`
    — interpolate `name` into `/{repo}/-/labels/{name}`.
  - The same pattern appears anywhere a method takes an identifier-
    like `String` argument and embeds it via `format!("/{…}/{}/…",
    arg)`. (Repos, issues, pulls all do this with structurally-safer
    arguments — numeric ids, slugs already validated upstream — but
    the convention is uniform.)
- **Summary**: SDK methods build URLs with
  `format!("/foo/{}/bar", arg).join_onto(base_url)`. If `arg`
  contains a `/`, the slash is interpreted as a *path separator* by
  `url::Url::join` rather than being percent-encoded as part of a
  single segment. The SDK does not validate or encode the input. So
  a label named `..` or `evil/../leak` is silently routed to a
  different endpoint instead of producing a clean error.

  This is **not exploitable** in practice for cnb.cool because the
  server-side router will simply 404 on garbage paths, but it does
  produce confusing error messages and turns a clean validation
  failure into a noisy "endpoint not found" 5 layers down the stack.
- **Workaround**: validate every user-controlled path component in
  the CLI before calling into the SDK. Helpers like
  `cnb-cli::commands::label::ensure_label_name_safe()` mirror the
  guards `cnb-api::services::labels::ensure_no_slash()` already had,
  returning `CliError::BadArgs` (exit 3) with a clear message
  pointing at the offending input.
- **Desired fix**: either:
  1. Have the SDK percent-encode each path segment when building the
     URL (the safe default — turns `evil/../leak` into a single
     literal segment), or
  2. Have the SDK reject components containing `/` with a typed
     error and document the constraint on every affected method.
- **Severity**: annoyance — easy enough to mirror the validation in
  every consumer, but it is the kind of thing that *should* be
  centralised so consumers do not silently disagree on what is
  rejected.

### SDK-I11 · `RepoPatch` is a strict subset of what `cnb repo edit` historically accepted

- **Surface**: `cnb::models::RepoPatch` (used by
  `RepositoriesClient::update_repo`)
- **Summary**: The typed PATCH body for a repository only carries
  `description` / `license` / `site` / `topics`. It does **not**
  include `name` or `default_branch`, both of which our hand-written
  `cnb-api::services::repos::EditRepoBody` did expose. We have no way
  to tell whether the upstream server silently dropped those fields
  before (the cnb-api code path serialised them anyway and ignored
  the response shape) or if they require a different endpoint
  (likely `set-default-branch` / `rename`, which the SDK does not
  yet model).
- **Workaround**: `cnb repo edit` rejects `--name` and
  `--default-branch` with a clear `BadArgs` (exit 3) pointing at the
  web UI for now, and accepts only `--description`. The CLI does
  NOT silently drop the flags — that would be worse than the
  facade's current behaviour.
- **Desired fix**: either (a) extend `RepoPatch` to include `name`
  and `default_branch` if the server actually supports them on the
  same `PATCH /{repo}` endpoint, or (b) generate dedicated typed
  methods (`rename_repo`, `set_default_branch`) for whatever
  endpoint the server actually exposes.
- **Severity**: annoyance — surfaces a real gap that the cnb-api
  facade was masking.

### SDK-I12 · `set_repo_visibility` uses a query string, not a body

- **Surface**: `cnb::repositories::RepositoriesClient::set_repo_visibility`
- **Summary**: The SDK builds `POST
  /{repo}/-/settings/set_visibility?visibility=public` with the
  visibility value as a query parameter. The hand-written cnb-api
  facade for the same endpoint sent a JSON body
  `{"visibility_level": 0}` instead. Both cannot be right; we have
  no integration coverage either way (no wiremock test against
  `set-visibility` in the legacy suite). Going with the SDK on the
  assumption it tracks the OpenAPI spec.
- **Workaround**: new `repo set-visibility` integration test is
  written against the SDK's request shape (query string). If a
  real cnb.cool server rejects the request, this row gets bumped
  to **blocker** and we fix in CLI by routing the call through
  `Context::sdk_raw_get`-style raw HTTP with a hand-built body.
- **Desired fix**: confirm with the server team which
  representation is canonical. Document on the SDK method.
- **Severity**: annoyance until proven by the wire — could be a
  blocker.

### SDK-I13 · `list_forks_repos` returns a wrapper object, not a `Vec`

- **Surface**: `cnb::repositories::RepositoriesClient::list_forks_repos`
  returning `cnb::models::ListForks { fork_tree_count, forks: Option<Vec<Forks>> }`
- **Summary**: Every other "list" method in the SDK returns
  `Vec<T>` directly. `list_forks_repos` is the odd one out: it
  wraps the slice in a struct that also carries a count. Not a bug
  per se — the upstream endpoint really does return that envelope —
  but it forces consumers to write `.forks.unwrap_or_default()`
  every time and breaks the otherwise-uniform pattern. Easy to miss
  on first read.
- **Workaround**: `cnb repo fork` unwraps `.forks` with a default
  empty vec so `--json` output stays an array (matching the cnb-api
  facade's previous behaviour and `gh repo fork`'s output shape).
- **Desired fix**: either rename the method to
  `get_fork_summary_repos` to make the wrapper expectation
  explicit, or expose a sibling `list_forks_repos_flat` that
  returns just the `Vec<Forks>`.
- **Severity**: polish.

### SDK-I14 · No non-JSON transport surface — bytes endpoints need a side-car reqwest client

- **Surface**: `cnb::http::HttpInner::{execute, execute_with_body}`
  - `execute::<T>(method, url)` — no body, decodes response as `T`
    via `serde_json::from_slice`.
  - `execute_with_body::<T, B>(method, url, body)` — attaches body
    with `req.json(b)` (sets `Content-Type: application/json`),
    decodes response as `T`.
  - `cnb::releases::ReleasesClient::get_releases_asset` returns
    `serde_json::Value` for an endpoint whose real response is
    either a 302 redirect to a pre-signed URL or the file bytes
    themselves — trying to JSON-decode bytes yields a parse error.
  - `cnb::build::BuildClient::build_runner_download_log` has the
    same problem: returns `serde_json::Value` for an endpoint
    whose body is plain text (`text/plain; charset=utf-8`), so
    a typed-only call always fails with `expected value at line
    1 column 1`.
- **Summary**: The SDK's shared HTTP layer is JSON-only on both
  the request and response legs. Three legitimate CNB flows need
  raw bytes today:
  1. **Two-phase release asset upload**, phase 2: `PUT
     <pre-signed upload_url>` with file bytes as the body. The SDK
     cannot express this — `execute_with_body` forces a JSON body.
  2. **Release asset download**: `GET /{repo}/-/releases/download/{tag}/{filename}`
     returns a 302 to a signed URL whose body is file bytes.
     `get_releases_asset` on the SDK decodes that body as
     `serde_json::Value` and fails on anything non-JSON.
  3. **Pipeline runner log download**: `GET /{repo}/-/build/runner/download/log/{pipelineId}`
     returns plain text (the raw runner log). `build_runner_download_log`
     on the SDK decodes as JSON and fails immediately on the first
     non-JSON byte.

  None of these flows can use the SDK's connection pool, retry
  logic, auth header, or tracing. They also cannot reuse the SDK's
  resolved token — the SDK does not expose the underlying
  `reqwest::Client` either (only `&HttpInner`, which has no
  `fn client()` accessor), so consumers re-read `CNB_TOKEN` from
  the env and build a fresh `reqwest::Client`.
- **Workaround**: `cnb-cli::commands::release` and
  `cnb-cli::commands::build` both keep the same small side-car
  pattern:
  - `release upload` phase 1 (`POST asset-upload-url`) → typed SDK
    via `post_release_asset_upload_url`.
  - `release upload` phase 2 (`PUT upload_url`) → standalone
    `reqwest::Client::new()` with
    `reqwest::Body::wrap_stream(ReaderStream::new(File::open))`.
    No auth header (pre-signed URL).
  - `release upload` phase 3 (`POST verify_url`) → reuses the
    SDK's `HttpInner::execute` because the verify endpoint *does*
    return JSON and the SDK's `execute(method, url)` accepts an
    arbitrary absolute `Url`.
  - `release download` and `build logs` → standalone
    `reqwest::Client::new()`, `CNB_TOKEN` read from env for
    `bearer_auth`. Same base-URL precedence
    (`CNB_API_BASE` > default) as the SDK to keep wiremock
    fixtures working.
- **Desired fix**: one of
  1. Add `pub fn HttpInner::reqwest_client(&self) -> &reqwest::Client`
     so consumers can build ad-hoc requests (bytes PUT, streaming
     GET) that still share the SDK's connection pool and base-URL
     machinery.
  2. Extend `execute_with_body` (or add a sibling) that accepts an
     arbitrary `reqwest::Body` instead of constraining the body to
     `Serialize`. Plus a streaming-response variant
     (`execute_bytes`) that returns `bytes::Bytes` instead of `T`.
  3. Model all three bytes endpoints as first-class typed methods
     (`upload_release_asset(path: impl AsRef<Path>)`,
     `download_release_asset(..) -> bytes::Bytes`,
     `download_runner_log(..) -> String`) so downstream code never
     reaches for the raw HTTP layer at all.
- **Severity**: annoyance — works via the side-car, but duplicates
  connection-pool + auth concerns across two HTTP clients for
  every `cnb release upload` / `cnb release download` / `cnb build logs`
  invocation. Three unrelated verbs share the same workaround,
  which makes the case for a generic fix stronger than any one of
  them alone.

### SDK-I15 · `list_package_tags` returns a single `Tag`, not `Vec<T>`

- **Surface**: `cnb::registries::RegistriesClient::list_package_tags`
  returning `cnb::models::Tag` (singular)
- **Summary**: The endpoint
  `GET /{slug}/-/packages/{type}/{name}/-/tags` emits an **array** of
  tag summaries (each with `name`, `updated_at`, etc.). But the SDK
  types it as returning `crate::models::Tag` — a single-object DTO
  that models a **git tag object**
  (`{commit, name, target, target_type, verification}`), which has
  nothing to do with registry package tags. The deserialisation
  will either fail (array → struct) or, if the server ever wraps
  the response in an object with a `name` field, silently pick up
  only that one field.

  This is the *second* SDK DTO that a CLI consumer cannot trust
  as-is, alongside SDK-I02 (`Repos4User` omits `default_branch`).
- **Workaround**: `cnb registry tag list` issues the typed call
  first (response discarded via `unwrap_or_default()` to exercise
  the SDK's request path — auth, base URL, retries, tracing) and
  then re-reads the endpoint via `Context::sdk_raw_get` to obtain
  the real array payload. The raw value is rendered through the
  normal `--json / --jq / --template / table` pipeline.
- **Desired fix**: rename the DTO or introduce a dedicated
  `RegistryPackageTag` / `RegistryPackageTagListItem` model and
  retype `list_package_tags` to return `Vec<…>`. The git-tag `Tag`
  DTO should stay on `git::GitClient` where it belongs.
- **Severity**: blocker for the typed path — every consumer of
  `list_package_tags` has to bypass the SDK's typed surface.

### SDK-I16 · `UpdateMembersRequest` body shape ≠ prior cnb-api facade shape

- **Surface**: `cnb::models::UpdateMembersRequest` used by
  `MembersClient::{add_members_of_group, update_members_of_group,
  add_members_of_repo, update_members_of_repo, ...}`
- **Summary**: The typed body for member add / edit is
  `{access_level: Option<String>, is_outside_collaborator: Option<bool>}`.
  The hand-written `cnb-api::services::orgs::AddMemberBody` /
  `EditMemberBody` we were replacing sent `{username, role}` on the
  POST and `{role}` on the PUT. The SDK's field name (`access_level`)
  also diverges from the one the CLI facade used to read from the
  response (`role`), so the display side had to be re-taught too.
  Similar to SDK-I12 (set-visibility), we have no integration
  evidence telling us which of the two representations the real
  server accepts today; we go with the SDK since it tracks the
  OpenAPI spec.
- **Workaround**: the CLI's `org member add/edit` forward
  `--role <value>` into `UpdateMembersRequest.access_level` verbatim
  and leave `is_outside_collaborator` as `None`. `member list`
  reads the typed `access_level` field directly — the legacy `role`
  key tolerance that the cnb-api facade had is gone.
- **Desired fix**: confirm the server's canonical body shape.
  Either:
  1. Document `access_level` as canonical and deprecate the
     `role` key on responses, or
  2. Add an `#[serde(alias = "role")]` on `access_level` so the
     SDK stays tolerant of servers still emitting the legacy key.
- **Severity**: annoyance — not blocking, but it's another "the
  wire disagrees with itself" case that puts the onus on every
  consumer to guess which side the server is on.

---

## Resolved issues

_(none yet)_

---

---

## Triage rules

When something breaks during a Phase 2 port, add an entry here instead
of fixing the SDK in-place. The goal is to:

1. Keep each CLI commit focused on one command port.
2. Accumulate a single well-written upstream patch / issue report when
   Phase 2 is done.
3. Give future readers a chronological record of "what weirdness we
   absorbed where and why".

If an issue turns out to block a port entirely, bump its severity to
**blocker** and raise it with upstream right away instead of waiting
for Phase 2 to finish.
