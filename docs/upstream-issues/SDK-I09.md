# `Pull.{head,base}` typed as `Option<serde_json::Value>` — every PR UI reinvents `read_branch`

> SDK ref: `cnb 0.2.x`
> Tracking id (consumer side): SDK-I09 — see
> [`cnb-cli` sdk-issues.md](https://…)

## TL;DR

The two most-frequently-rendered fields on a pull request —
`head.branch` and `base.branch` — are typed as
`Option<serde_json::Value>` on the SDK's `Pull` and
`PullRequest` DTOs. The OpenAPI spec doesn't pin the schema, so
the SDK correctly refuses to commit to a shape. The catch:
**downstream consumers still have to extract a branch name to
render anything useful**, and the real cnb.cool API returns at
least three different shapes across deployments.

The result: every PR UI ships its own `read_branch` helper that
tries 3-4 keys in priority order. That's roughly 50% of all PR-
rendering code in our consumer.

## Surface area

- `cnb::models::Pull.head: Option<serde_json::Value>`
- `cnb::models::Pull.base: Option<serde_json::Value>`
- `cnb::models::PullRequest.head: Option<serde_json::Value>`
- `cnb::models::PullRequest.base: Option<serde_json::Value>`

(Returned by `PullsClient::get_pull` and
`PullsClient::list_pulls` respectively. SDK-I08 separately
tracks the fact that these are *two different DTOs* for the
same resource.)

## Observed wire shapes

In production traffic and our own test fixtures we have seen
all four of these (sometimes from the same server, depending on
endpoint):

```json
// shape 1 — current default
{"head": {"branch": "feat/x", "commit_id": "abc1234"}}

// shape 2 — git-ref form
{"head": {"ref": "refs/heads/feat/x"}}

// shape 3 — name-form
{"head": {"name": "feat/x"}}

// shape 4 — legacy top-level sibling field
{"source_branch": "feat/x", "target_branch": "main"}
```

Shape 4 is particularly painful because `source_branch` /
`target_branch` are not modeled on the DTO at all — even with
`head: Option<Value>`, they're discarded by the time the call
returns. The consumer has to do the raw GET in parallel to
recover them.

## Minimal reproduction

```rust
use cnb::pulls::PullsClient;
use serde_json::Value;

async fn show_branches(c: &PullsClient, repo: &str, n: &str) -> Result<()> {
    let pr = c.get_pull(repo, n.to_string()).await?;

    // pr.head: Option<Value>. Try each known shape in order.
    let head = pr.head.as_ref()
        .and_then(|v| {
            for key in ["branch", "ref", "name"] {
                if let Some(s) = v.get(key).and_then(Value::as_str) {
                    return Some(s.to_string());
                }
            }
            None
        })
        .unwrap_or_default();

    println!("head = {head}");
    Ok(())
}
```

…and the same logic for `base`, plus a fallback path for the
legacy sibling fields. Our consumer's helper, factored out at
`crates/cnb-cli/src/commands/pr.rs:371` (commit
[`b785d35`](https://…)):

```rust
fn read_branch(primary: Option<&Value>, fallback: Option<&Value>) -> String {
    if let Some(obj) = primary {
        for key in ["branch", "ref", "name"] {
            if let Some(s) = obj.get(key).and_then(Value::as_str) {
                return s.to_string();
            }
        }
    }
    if let Some(s) = fallback.and_then(Value::as_str) {
        return s.to_string();
    }
    String::new()
}

// Tested with 5 cases (`branch`/`ref`/`name`/legacy/empty),
// invoked from `pr list`, `pr view`, and `pr checkout`.
```

It's a 30-line helper. Each consumer will need to write
roughly the same one.

## Suggested fix

Settle the spec on one canonical encoding and promote it to a
real DTO. Our recommendation, based on what the current default
deployment emits:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRef {
    /// Short branch name, e.g. "feat/x".
    pub branch: Option<String>,
    /// Commit SHA at the tip of the branch when the PR was
    /// recorded. Optional because some events don't have one.
    pub commit_id: Option<String>,
}

// then on Pull / PullRequest:
pub head: Option<PullRef>,
pub base: Option<PullRef>,
```

That turns the consumer's 30-line helper into a one-liner:

```rust
let head = pr.head.as_ref()
    .and_then(|r| r.branch.as_deref())
    .unwrap_or_default();
```

If transition for older deployments is a concern, an
`#[serde(alias = "ref", alias = "name")]` attribute on `branch`
would tolerate the historical key variants without breaking the
typed shape.

For the **legacy top-level `source_branch` / `target_branch`**
case (shape 4 above): worth confirming whether any current
cnb.cool deployment still relies on it. If yes, the SDK can
expose a sibling `source_branch` / `target_branch` on
`Pull` / `PullRequest`, both `Option<String>`, and document
that they're populated only when the corresponding `head` /
`base` object is missing.

## Severity

Blocker-adjacent — not for compilation, but for any UI that
wants branch info on a PR. **Every consumer has to reinvent
`read_branch`**, including the cross-product with the legacy
sibling fields the SDK doesn't model at all.

## Related

- SDK-I08 — `Pull` vs `PullRequest` DTO divergence (the same
  `head` / `base` field shape on both).
