# Polish & conventions — meta-issue from the cnb-cli port

<!-- markdownlint-disable MD024 MD031 MD060 -->
<!-- MD024: each sub-issue intentionally reuses the same H3 template
     for consistency with Tier-B.md. MD031/MD060: code fences inside
     numbered lists and aligned-table widening would hurt copy-paste
     readability. -->

> SDK ref: `cnb 0.2.x` (alias `cnb-sdk` in the consumer workspace)
> Tracking ids (consumer side): SDK-I04, SDK-I05, SDK-I06, SDK-I10,
> SDK-I12, SDK-I16, SDK-I17, SDK-I18 — see
> [`cnb-cli` sdk-issues.md](https://…)
> Anchor commit: `b785d35` (Phase 2 step 2.11)

## TL;DR

Eight small housekeeping items that surfaced during the
cnb-cli typed-SDK port. Each one alone wouldn't warrant a
ticket; together they're worth one well-written meta-issue so
the maintainer can triage them as a group.

| Sub-id   | Subgroup                    | Severity   | One-liner                                                                                  |
|----------|-----------------------------|------------|--------------------------------------------------------------------------------------------|
| SDK-I04  | Publishing metadata         | polish     | Crate name `cnb` collides with any binary also named `cnb`                                 |
| SDK-I06  | Publishing metadata         | polish     | `repository` URL in crate metadata 404s on unauthenticated visit                           |
| SDK-I05  | Generated-code conventions  | polish     | Query structs lack `#[non_exhaustive]`; direct-init becomes a SemVer hazard                |
| SDK-I10  | Defensive defaults          | annoyance  | No path-segment validation/encoding for user-controlled identifiers                        |
| SDK-I12  | Spec / server alignment     | annoyance† | `set_repo_visibility` uses query string, not body — unclear which is canonical             |
| SDK-I16  | Spec / server alignment     | annoyance† | `UpdateMembersRequest` body shape disagrees with prior facade; spec vs wire unclear        |
| SDK-I17  | Spec / query completeness   | annoyance  | `GetRepoContributorTrendQuery` omits the `days` filter the server accepts                  |
| SDK-I18  | Missing verbs               | annoyance  | `pinned-repos` PUT endpoint is not modeled (only the GET half is generated)                |

† SDK-I12 and SDK-I16 are tagged "annoyance until proven by the
wire — could be a blocker". They depend on a server-team
clarification we don't have integration evidence for either way.

---

## Subgroup 1 · Publishing metadata (SDK-I04, SDK-I06)

### SDK-I04 · `cnb` package name collides with downstream `cnb` binaries

Our binary is also named `cnb`. Depending directly on `cnb =
"0.2"` creates an ambiguous package name in the same workspace.
Every cargo command that would otherwise accept `-p cnb` has to
be disambiguated (`cargo test -p cnb@0.4.0-alpha.1` or the
equivalent `--manifest-path` form).

**Workaround**: rename the dependency to `cnb-sdk` via cargo's
`package =` field:

```toml
# Cargo.toml
[workspace.dependencies]
cnb-sdk = { package = "cnb", version = "0.2", … }
```

All in-code references use `cnb_sdk::…`. Integration tests run
via `--manifest-path crates/cnb/Cargo.toml`. Stable, but the
rename noise shows up in every Cargo.toml and every `use` line
of every consumer.

**Why this is worth fixing upstream**: any CLI or workspace whose
bin target coincides with the crate name has to repeat this
dance. It's not an uncommon case for an API client crate (the
client is often named after the service it talks to, and the
service's first-party CLI uses the same name).

**Suggested fix**: republish as `cnb-sdk` or `cnb-client` on
crates.io. The current crate could remain as a thin re-export
shim during transition.

### SDK-I06 · `repository` URL in crate metadata 404s

The crate metadata points at
`https://cnb.cool/aodoo/tools/rust-cnb.git` as the repository.
The corresponding web URL returns 404 ("页面不存在或访问权限不足")
on an unauthenticated visit. docs.rs makes up for most of it,
but anyone following the crates.io "Repository" link lands on a
dead page.

**Workaround**: read `docs.rs/cnb/0.2.1/cnb/` instead. The
README that ships inside the `.crate` tarball has the useful
bits.

**Suggested fix**: either make the repo public-read or redirect
the Cargo.toml `repository` field to a browsable mirror
(GitHub / source.gc / etc.).

This also blocks anything that wants to reference upstream
issues from a downstream project — there's no clean URL to
hyperlink to. The `https://…` placeholders littered through
**Tier A**, **Tier B**, and this document are all waiting on
this fix.

---

## Subgroup 2 · Generated-code conventions (SDK-I05)

### SDK-I05 · No `#[non_exhaustive]` on query structs

Query parameter structs (`GetReposQuery`,
`GetReposByUserNameQuery`, `GetGroupSubReposQuery`, etc.) are
plain `#[derive(Default)]` with public fields and no
`#[non_exhaustive]`. If upstream adds a new optional query
field in a future minor version, any consumer that built the
struct with positional / pattern-match init would break.

Builder methods exist (`q.page(n).page_size(m)`) and we use
them everywhere, so we are fine today. But the escape hatch of
direct struct init compiles, and tempts users into a SemVer
hazard.

**Workaround**: prefer the builder chain
(`GetReposQuery::new().page(…).page_size(…)`) in all CLI code;
avoid `GetReposQuery { page: ..., ..Default::default() }` even
though it compiles. Documented in our internal contributor
notes.

**Suggested fix**: add `#[non_exhaustive]` to every query /
body struct, or mark fields `pub(crate)` and force builder-only
access. The latter is the stronger contract; the former is the
smaller diff.

---

## Subgroup 3 · Defensive defaults (SDK-I10)

### SDK-I10 · No path-segment validation on user-controlled identifiers

SDK methods build URLs with `format!("/foo/{}/bar",
arg).join_onto(base_url)`. If `arg` contains a `/`, the slash is
interpreted as a path separator by `url::Url::join` rather than
being percent-encoded as part of a single segment. The SDK does
not validate or encode the input.

**Concretely observed** on:

- `cnb::repo_labels::RepoLabelsClient::{delete_label,
  patch_label}` — interpolate `name` into
  `/{repo}/-/labels/{name}`.
- The same pattern appears anywhere a method takes an
  identifier-like `String` argument and embeds it via
  `format!`. Repos, issues, pulls all do this with
  structurally-safer arguments (numeric ids, slugs already
  validated upstream — but the convention is uniform).

This is **not exploitable** for cnb.cool because the server-side
router will simply 404 on garbage paths, but it does produce
confusing error messages and turns a clean validation failure
into a noisy "endpoint not found" 5 layers down the stack.

**Workaround**: validate every user-controlled path component
in the CLI before calling into the SDK. Helpers like
`cnb-cli::commands::label::ensure_label_name_safe()` mirror the
guards `cnb-api::services::labels::ensure_no_slash()` already
had, returning `CliError::BadArgs` (exit 3) with a clear
message pointing at the offending input.

**Suggested fix**: pick one of:

1. Have the SDK percent-encode each path segment when
   building the URL (the safe default — turns
   `evil/../leak` into a single literal segment).
2. Have the SDK reject components containing `/` with a typed
   error and document the constraint on every affected method.

Whichever is chosen, the goal is to centralise the rule so
consumers don't silently disagree on what gets rejected.

---

## Subgroup 4 · Spec / server alignment (SDK-I12, SDK-I16)

These two share a structure: the SDK and the prior cnb-api
facade emit different shapes for the same endpoint, and **we
have no integration evidence** telling us which one the real
server accepts today. Both are tagged "annoyance until proven
by the wire — could be a blocker".

### SDK-I12 · `set_repo_visibility` uses a query string, not a body

The SDK builds:

```http
POST /{repo}/-/settings/set_visibility?visibility=public
```

with the visibility value as a **query parameter**. The
hand-written cnb-api facade for the same endpoint sent a JSON
body `{"visibility_level": 0}` instead. Both cannot be right.

**Workaround**: the new `repo set-visibility` integration test
is written against the SDK's request shape (query string). If
a real cnb.cool server rejects the request, this row gets
bumped to **blocker** and we fix it in the CLI by routing the
call through `Context::sdk_raw_json(POST, path, body)` with a
hand-built JSON body.

**Suggested fix**: confirm with the server team which
representation is canonical. Document on the SDK method.

### SDK-I16 · `UpdateMembersRequest` body shape ≠ prior facade shape

The typed body for member add / edit is:

```rust
UpdateMembersRequest {
    access_level: Option<String>,
    is_outside_collaborator: Option<bool>,
}
```

The hand-written `cnb-api::services::orgs::AddMemberBody` /
`EditMemberBody` we replaced sent `{username, role}` on the
POST and `{role}` on the PUT.

Two divergences:

1. **Field name**: `access_level` (SDK) vs `role` (facade).
2. **Identity carrier**: SDK has no `username` field; the
   facade sent it inside the body, presumably as a user
   reference.

The SDK's field name (`access_level`) also diverges from the one
the CLI facade used to read from the response (`role`), so the
display side had to be re-taught too.

**Workaround**: `org member add/edit` forward `--role <value>`
into `UpdateMembersRequest.access_level` verbatim and leave
`is_outside_collaborator` as `None`. `member list` reads the
typed `access_level` field directly — the legacy `role` key
tolerance that the cnb-api facade had is gone.

**Suggested fix**: confirm the server's canonical body shape.
Either:

1. Document `access_level` as canonical and deprecate the
   `role` key on responses, or
2. Add `#[serde(alias = "role")]` on `access_level` so the
   SDK stays tolerant of servers still emitting the legacy
   key on either request or response.

---

## Subgroup 5 · Spec / query completeness (SDK-I17)

### SDK-I17 · `GetRepoContributorTrendQuery` omits the `days` filter

The server accepts `?days=N` on
`GET /{slug}/-/contributor/trend` — the cnb-api facade and the
CLI have documented a `--days` flag since M4 launch. The SDK's
typed query struct supports `limit` and
`exclude_external_users` but **not** `days`, so a typed-only
call cannot reach that filter. The existing `limit` field is a
result-count cap, not a time window — different semantics.

**Workaround**: `cnb repo contributors` routes through
`Context::sdk_raw_get` with `?days=N` appended when the user
passes `--days`. Without `--days`, the typed
`get_repo_contributor_trend` path is used unchanged.

**Suggested fix**: add `days: Option<i64>` (with a
corresponding builder method) to
`GetRepoContributorTrendQuery`. Confirm the spec mentions
`days` — if not, file the missing parameter upstream as a spec
gap as well.

---

## Subgroup 6 · Missing verbs (SDK-I18)

### SDK-I18 · `pinned-repos` PUT endpoint is not modeled

`cnb::repositories::RepositoriesClient` exposes:

- `get_pinned_repo_by_group` (GET, by group slug)
- `get_pinned_repo_by_id` (GET, by user id)

…but **not** the `PUT /{slug}/-/pinned-repos` endpoint that
replaces the pinned set.

**Workaround**: the CLI does a typed GET via
`get_pinned_repo_by_group` and a raw PUT via
`Context::sdk_raw_json(PUT, path, body)`. We added
`sdk_raw_json` specifically to unblock this case — it routes
through the SDK's `HttpInner::execute_with_body` so the request
still shares the SDK's retry / auth / tracing setup.

**Suggested fix**: generate
`set_pinned_repos(slug, body: &SetPinnedRepos)` from the
OpenAPI spec. Body shape: `{repos: Vec<String>}`. Returns
`serde_json::Value` (or a typed ack DTO if the server spec
exposes one).

This is also the cleanest "add a method" patch we have on the
list — if the OpenAPI spec already documents the endpoint, we
can offer a PR for it directly.

---

## Suggested handling

These eight don't need separate tickets, but they are all
real friction. Three handling strategies, in order of
maintainer effort:

1. **Triage close**: most of these can land in a single
   "polish" point release.
2. **Spec confirmation needed for two**: SDK-I12 and SDK-I16
   need a server-team yes/no before the SDK can converge.
   Until then we live with the SDK's chosen shape.
3. **Patch ready**: SDK-I04 (republish), SDK-I05
   (`#[non_exhaustive]`), SDK-I06 (URL fix), SDK-I18
   (`pinned-repos` PUT) are all small, mechanical changes.
   Happy to send PRs against the upstream once the canonical
   mirror URL is confirmed (see SDK-I06).

---

## Anchors

All workarounds above can be inspected at commit
[`b785d35`](https://…) of the cnb-cli consumer:

- `Cargo.toml` — `cnb-sdk = { package = "cnb", … }` (SDK-I04).
- `crates/cnb-cli/src/commands/repo.rs` — `sdk_raw_get` for
  `--days` (SDK-I17), pin/unpin via `sdk_raw_json` (SDK-I18).
- `crates/cnb-cli/src/commands/label.rs` —
  `ensure_label_name_safe` (SDK-I10).
- `crates/cnb-cli/src/commands/org.rs` — `UpdateMembersRequest`
  call sites (SDK-I16).
- `crates/cnb/tests/m2_repo.rs` — wiremock tests for the SDK's
  query-string `set_visibility` shape (SDK-I12).
