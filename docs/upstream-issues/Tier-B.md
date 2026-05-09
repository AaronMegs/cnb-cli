# DTO completeness & method-signature consistency during the cnb-cli port

<!-- markdownlint-disable MD024 MD031 MD060 -->
<!-- MD024: each sub-issue intentionally reuses the same H3 sub-headings
     (Surface / What goes wrong / Workaround / Suggested fix) as a
     consistent template — this is a structural choice, not duplication.
     MD031/MD060: code fences inside numbered lists and aligned-table
     widening would hurt copy-paste readability. -->

> SDK ref: `cnb 0.2.x` (alias `cnb-sdk` in the consumer workspace)
> Tracking ids (consumer side): SDK-I01, SDK-I02, SDK-I08, SDK-I11,
> SDK-I13, SDK-I19 — see
> [`cnb-cli` sdk-issues.md](https://…)
> Anchor commit: `b785d35` (Phase 2 step 2.11)

## TL;DR

A bundle of six closely-related friction points discovered while
porting `cnb-cli` from a hand-written facade to the typed SDK.
None of them block compilation. Together they cost the consumer
non-trivial boilerplate around a single underlying theme: the
generated DTOs and method signatures occasionally diverge from
what the server actually accepts / returns, in ways that force
every downstream caller to reinvent the same workaround.

We're filing them as one issue (rather than six) because:

- Each one alone is small.
- They are best fixed together as a "DTO polish pass" rather
  than on a one-by-one schedule.
- A single PR could land most of them.

| Sub-id   | Theme                          | Severity | Workaround pattern                          |
|----------|--------------------------------|----------|---------------------------------------------|
| SDK-I01  | Method visibility              | annoyance | `Context::sdk_raw_get` re-derives base URL  |
| SDK-I02  | DTO completeness (Repos4User)  | annoyance | dual GET (typed + raw)                      |
| SDK-I08  | DTO completeness (Pull twins)  | annoyance | render via `serde_json::Value`              |
| SDK-I11  | DTO completeness (RepoPatch)   | annoyance | CLI rejects `--name` / `--default-branch`   |
| SDK-I13  | Signature consistency (forks)  | polish    | unwrap wrapper to flat `Vec`                |
| SDK-I19  | DTO completeness (PR writes)   | annoyance | CLI rejects 4 flags as BadArgs              |

---

## SDK-I01 · `HttpInner::url()` and `base_url` are `pub(crate)`

### Surface

`cnb::http::HttpInner`

### What goes wrong

`HttpInner::url(path)` and the `base_url: Arc<Url>` field are
`pub(crate)`, so downstream callers cannot reach the
already-resolved base URL to construct ad-hoc requests through
the SDK's own HTTP layer. `HttpInner::execute::<T>` is `pub` and
accepts an arbitrary `Url`, but **nobody outside the crate can
build that `Url` correctly** (base-URL precedence, trailing-slash
normalisation, percent-encoding) without duplicating the SDK's
logic.

### Workaround

`cnb-cli::Context::sdk_raw_get(path)` recomputes the base URL
from its own sources (`CNB_API_BASE` > SDK default) and uses
`url::Url::parse().join()` to build the final URL, then hands it
to `HttpInner::execute::<Value>(GET, url)`. Works, but the CLI
now has to know two separate things the SDK already knows: the
base URL, and the "trailing slash" rule for `Url::join`.

### Suggested fix

Pick any one of:

1. **Make the existing method public**:
   ```rust
   pub fn HttpInner::url(&self, path: &str) -> Result<Url, …>;
   ```
2. **Expose the base URL**:
   ```rust
   pub fn ApiClient::base_url(&self) -> &Url;
   ```
   plus a documented `url::Url::join()` contract.
3. **Add a higher-level convenience**:
   ```rust
   pub async fn HttpInner::get_json<T>(&self, path: &str) -> Result<T, …>;
   ```
   that wraps `url()` + `execute::<T>()`.

Option 1 is the smallest patch. Pairs naturally with **SDK-I14**
(non-JSON transport), which also wants a `reqwest::Client`
accessor on `HttpInner`.

---

## SDK-I02 · `Repos4User` DTO omits server-side fields

### Surface

`cnb::models::Repos4User`, returned by
`repositories::get_by_id`.

### What goes wrong

`Repos4User` does not model every field the real server returns
for `GET /{repo}`. **Concrete example**: production responses
include `default_branch`, but the DTO has no such field. A
typed-only call silently drops it; `--json` output then lies to
users.

```text
GET /ORG/REPO 200
{"path":"ORG/REPO","name":"REPO","default_branch":"main","visibility_level":"public",…}
                                     ^^^^^^^^^^^^^^^^^^^^^^^ — typed DTO drops this
```

### Workaround

`cnb repo view` fetches the endpoint twice:

1. Typed call (catches schema regressions early).
2. Raw `serde_json::Value` call (preserves every field for
   `--json` / `--jq` / `--template`).

Both calls share the SDK's reqwest connection pool so the
extra cost is one round-trip on a single-object GET. Wasteful
but cheap.

### Suggested fix

- Add `default_branch` (and any other verified-present fields)
  to `Repos4User`.
- **Longer term**: generate the DTO directly from the server's
  OpenAPI spec with a `#[serde(flatten)] extra: HashMap<String,
  Value>` catch-all so undocumented fields survive by default
  rather than getting dropped.

---

## SDK-I08 · `Pull` and `PullRequest` are two DTOs for the same resource

### Surface

`cnb::pulls::PullsClient`:

- `get_pull()` returns `cnb::models::Pull`.
- `list_pulls()` returns `Vec<cnb::models::PullRequest>`.

### What goes wrong

Two generated structs for the same underlying resource, with
overlapping but subtly different field sets:

| field          | `Pull`              | `PullRequest`               |
|----------------|---------------------|-----------------------------|
| `labels`       | `Vec<LabelInfo>`    | `Vec<Label>`                |
| `comment_count`| absent              | `Option<i64>`               |
| `review_count` | absent              | `Option<i64>`               |
| `repo`         | absent              | `Option<serde_json::Value>` |
| `created_at`   | absent              | `Option<String>`            |
| `last_acted_at`| absent              | `Option<String>`            |
| `reviewers`    | `Vec<PullReviewer>` | absent                      |
| `updated_at`   | `Option<String>`    | `Option<String>`            |

This mirrors the upstream spec's distinction between "detail
view" and "list item" — valid in theory, but painful in
practice: a CLI that wants to share rendering code between
`list` and `view` either has to pick the lowest common
denominator (and lose typed access to a handful of fields) or
deal with two parallel `match` arms.

### Workaround

Convert both DTOs to `serde_json::Value` at the rendering
boundary in `commands::pr::{list, view}` and read every field
through `Value::get()`. This loses the benefit of typed access
on the hot path, but keeps a single render function.

### Suggested fix

- **Preferred**: generate a single `Pull` struct with an
  optional `stats: Option<PullStats>` sub-struct (or similar)
  for the list-only bean counters.
- **Alternative**: `#[serde(flatten)]` the shared portion into
  both, so downstream code can destructure a common view.

Cross-reference with the `Issue` / `IssueDetail` divergence,
which has the exact same shape — a single decision could fix
both.

---

## SDK-I11 · `RepoPatch` is a strict subset of what `cnb repo edit` historically accepted

### Surface

`cnb::models::RepoPatch` (used by
`RepositoriesClient::update_repo`).

### What goes wrong

The typed PATCH body for a repository carries only
`description` / `license` / `site` / `topics`. It does **not**
include `name` or `default_branch`, both of which our
hand-written `cnb-api::services::repos::EditRepoBody` exposed.

We don't know whether the server silently dropped those fields
before (the cnb-api code path serialised them anyway and ignored
the response shape) or whether they require a different endpoint
(plausibly `set-default-branch` / `rename`, which the SDK does
not yet model either way).

### Workaround

`cnb repo edit` rejects `--name` and `--default-branch` upfront
with a clear `BadArgs` (exit 3) message pointing at the web UI,
and accepts only `--description`. The CLI does **not** silently
drop the flags — that would be worse than the facade's previous
behaviour, which at least sent something on the wire.

### Suggested fix

Either of:

1. **Extend `RepoPatch`** to include `name` and
   `default_branch` if the server actually supports them on the
   same `PATCH /{repo}` endpoint.
2. **Generate dedicated typed methods** (`rename_repo`,
   `set_default_branch`) for whatever endpoint the server
   actually exposes.

Document the choice on the method either way — currently a
consumer reading the SDK has no signal that "rename" / "change
default branch" are unavailable through the typed surface.

---

## SDK-I13 · `list_forks_repos` returns a wrapper object, not a `Vec<Forks>`

### Surface

```rust
RepositoriesClient::list_forks_repos(slug, …)
    -> ListForks { fork_tree_count: i64, forks: Option<Vec<Forks>> }
```

### What goes wrong

Every other "list" method in the SDK returns `Vec<T>`
directly. `list_forks_repos` is the odd one out: it wraps the
slice in a struct that also carries a count.

Not a bug per se — the upstream endpoint really does return
that envelope — but it forces consumers to write
`.forks.unwrap_or_default()` every time and breaks the
otherwise-uniform pattern. Easy to miss on first read.

### Workaround

`cnb repo fork` unwraps `.forks` with a default empty vec so
`--json` output stays an array (matching the cnb-api facade's
previous behaviour and `gh repo fork`'s output shape).

### Suggested fix

Either of:

1. **Rename the method** to `get_fork_summary_repos` to make
   the wrapper expectation explicit at the call site.
2. **Expose a sibling** `list_forks_repos_flat` that returns
   just the `Vec<Forks>`, leaving the existing wrapper variant
   for callers that actually need `fork_tree_count`.

---

## SDK-I19 · PR write-path DTOs miss CLI-relevant fields

### Surface

- `cnb::models::PullCreationForm` — has `title`, `head`, `base`,
  `body`, `head_repo`. **Missing**: `assignees`, `labels`.
- `cnb::models::PatchPullRequest` — has `title`, `body`, `state`.
  **Missing**: `base` (no way to retarget a PR to a different
  base branch).
- `cnb::models::MergePullRequest` — has `merge_style`,
  `commit_title`, `commit_message`. **Missing**:
  `remove_source_branch` (the "delete branch on merge" toggle).
- The `merge_method` key the cnb-api facade used previously is
  renamed to `merge_style` on the SDK; the wire shape has not
  been independently confirmed.

### What goes wrong

Three independent gaps in the typed PR write surface — but they
all share a single root cause (incomplete generated body
structs) and a single root fix shape (add the missing fields).
The cnb-api facade serialised `assignees` / `labels` /
`target_branch` (= base) / `remove_source_branch` anyway and
silently relied on the server to either honour or drop them; the
SDK pins each form to a strict shape, so a typed-only call cannot
forward those fields at all. Whether the server actually accepts
any of them with no schema entry is unknown — we have no
integration evidence either way.

### Workaround

`cnb pr create --assignee` / `--label`, `cnb pr edit --base`,
and `cnb pr merge --delete-branch` are now rejected at the CLI
layer with a `BadArgs` (exit 3) message pointing the user at
the composable alternative:

- `pr create --assignee USER` → `pr create` then
  `pr assign --add USER`.
- `pr create --label LABEL` → `pr create` then
  `pr label --add LABEL`.
- `pr edit --base BRANCH` → not currently expressible.
- `pr merge --delete-branch` → delete the source branch as a
  separate post-merge step (a simple `git push origin
  --delete <branch>` or the `cnb branch delete` equivalent).

This surfaces the gap rather than letting the typed call
silently drop the user's intent.

### Suggested fix

1. Add `assignees: Option<Vec<String>>` and
   `labels: Option<Vec<String>>` to `PullCreationForm`.
2. Add `base: Option<String>` to `PatchPullRequest` (the
   `/{repo}/-/pulls/{number}` PATCH endpoint should support
   retargeting; if the server truly does not, document the
   restriction on the method instead).
3. Add `remove_source_branch: Option<bool>` to
   `MergePullRequest`. Confirm the wire field name —
   `merge_style` vs `merge_method` — and document the
   canonical one on the SDK method.

---

## How we'd like to land this

If a single "DTO polish PR" against the upstream `cnb` crate is
on the table, our suggested order (smallest diff first):

1. **SDK-I01** (visibility flip) — single keyword change.
2. **SDK-I02** (`default_branch` on `Repos4User`) — one field.
3. **SDK-I19** (4 missing fields across 3 PR write DTOs) — all
   `Option<…>`, additive.
4. **SDK-I11** (`name` / `default_branch` on `RepoPatch` *or*
   new methods).
5. **SDK-I13** (rename or sibling on `list_forks_repos`).
6. **SDK-I08** (Pull vs PullRequest unification) — biggest
   surface change; warrants its own discussion.

Happy to draft a PR for items 1–3 against the upstream repo if
that helps move things along. Let us know which mirror is the
canonical one for contributing back (see SDK-I06 in
[Tier C](./Tier-C.md)).

---

## Anchors

All workarounds above can be inspected at commit
[`b785d35`](https://…) of the cnb-cli consumer:

- `crates/cnb-cli/src/context.rs` — `sdk_raw_get`,
  `sdk_raw_json` (SDK-I01).
- `crates/cnb-cli/src/commands/repo.rs` — dual GET in
  `repo view`; `repo edit` flag rejection (SDK-I02, SDK-I11).
- `crates/cnb-cli/src/commands/pr.rs` — `Value`-based render;
  PR write flag rejections (SDK-I08, SDK-I19).
- `crates/cnb-cli/src/commands/repo.rs` — fork list unwrap
  (SDK-I13).
