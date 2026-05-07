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

### SDK-I07 · Issue number type is inconsistent across the SDK surface

- **Surface**: `cnb::issues::IssuesClient::get_issue(repo, number: i64)`
  vs `cnb::models::{Issue, IssueDetail, UserIssue}.number: Option<String>`
- **Summary**: The typed `get_issue` method takes `number: i64`, yet the
  DTO it returns (and every other issue DTO) types `number` as
  `Option<String>`. The same mismatch appears on the related endpoints
  (`list_issue_comments`, `get_issue_comment`, `update_issue`, etc — all
  take `i64` arguments but return string-valued `number`s). That forces
  every CLI consumer to convert between the two at the boundary and
  pushes an implicit assertion about what formats of issue number the
  server accepts.
- **Workaround**: `cnb-cli::commands::issue` passes `i64` to
  `get_issue`, converting from the CLI's `u64` parameter type through
  `i64::try_from`. The CLI display layer uses
  `format_issue_number(Option<&Value>)` which tolerates both string and
  integer encodings on the response side.
- **Desired fix**: pick one representation and apply it everywhere. If
  the spec really does want strings (e.g. to support sparse / scoped
  numbering), change the method arguments to `&str` / `String`.
  Otherwise, normalise the DTO to `Option<i64>`.
- **Severity**: annoyance — the conversion dance is contagious: any
  downstream code that threads issue numbers through its own layers
  has to pick one side and stick with it.

---

## Resolved issues

_(none yet)_

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
