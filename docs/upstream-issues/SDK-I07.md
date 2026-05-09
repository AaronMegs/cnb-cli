# Issue / Pull number type is inconsistent across the SDK surface

> SDK ref: `cnb 0.2.x`
> Tracking id (consumer side): SDK-I07 — see
> [`cnb-cli` sdk-issues.md](https://…)

## TL;DR

The same conceptual identifier — "the numeric handle of an
issue or a pull request" — is typed three different ways on the
SDK surface, and the two clients (`IssuesClient` and
`PullsClient`) disagree with each other:

|                                      | Argument type | DTO field type   |
|--------------------------------------|---------------|------------------|
| `IssuesClient::get_issue`            | `i64`         | `Option<String>` |
| `IssuesClient::post_issue_assignees` | `String`      | `Option<String>` |
| `PullsClient::get_pull`              | `String`      | `Option<String>` |

So:

- Within `issues`: argument is `i64`, response field is `String`,
  **except** the assignee endpoints which take `String`.
- Within `pulls`: both argument and response field are `String`.
- Cross-module: the same kind of "numeric resource id" is `i64`
  on one client and `String` on the other.

This forces every consumer to convert at least once and pick a
favourite for any code that wants to thread issue numbers
through its own types.

## Surface area

Selected examples (the pattern is uniform across each family):

- `cnb::issues::IssuesClient`
  - `get_issue(repo, number: i64) -> Issue`
  - `update_issue(repo, number: i64, &PatchIssueForm) -> …`
  - `post_issue_comment(repo, number: i64, …) -> …`
  - `patch_issue_comment(repo, number: i64, comment_id: i64, …)`
  - `list_issue_comments(repo, number: i64, …) -> …`
  - `list_issue_activities(repo, number: i64, …) -> …`
  - `update_issue_properties(repo, number: i64, …) -> …`
  - **but** `post_issue_assignees(repo, number: String, …) -> …`
  - **but** `delete_issue_assignees(repo, number: String, …) -> …`
- `cnb::pulls::PullsClient`
  - `get_pull(repo, number: String) -> Pull`
  - `patch_pull(repo, number: String, &PatchPullRequest) -> …`
  - `post_pull(repo, &PullCreationForm) -> …`
  - `list_pulls_by_numbers(repo, &…) -> …`
- `cnb::models`
  - `Issue.number: Option<String>`
  - `IssueDetail.number: Option<String>`
  - `Pull.number: Option<String>`
  - `PullRequest.number: Option<String>`

## Minimal reproduction (the conversion dance)

```rust
use cnb::issues::IssuesClient;

async fn render(client: &IssuesClient, repo: &str, n_input: u64) -> Result<()> {
    // 1. CLI receives `u64` from clap. SDK wants `i64`. Convert.
    let n_i64: i64 = i64::try_from(n_input)
        .map_err(|_| "issue number out of range")?;

    // 2. Read the issue. SDK call takes the `i64`.
    let issue = client.get_issue(repo, n_i64).await?;

    // 3. The DTO's `number` is `Option<String>`. To display, parse it
    //    back to a number (and fall back if it isn't even numeric).
    let n_for_display = issue.number
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());

    println!("issue #{n_for_display}");
    Ok(())
}
```

```rust
use cnb::issues::IssuesClient;

async fn assign(client: &IssuesClient, repo: &str, n_input: u64) -> Result<()> {
    // Same issue, same client — but the assignee endpoint disagrees
    // with `get_issue` about the number type. Now we need a String.
    let n_str = n_input.to_string();
    client.post_issue_assignees(repo, n_str, &/*…*/).await?;
    Ok(())
}
```

```rust
use cnb::pulls::PullsClient;

async fn render_pr(client: &PullsClient, repo: &str, n_input: u64) -> Result<()> {
    // Pulls are internally consistent (String everywhere), but
    // disagree with issues. So the same `u64` from clap takes a
    // different conversion path here.
    let n_str = n_input.to_string();
    let pr = client.get_pull(repo, n_str).await?;
    Ok(())
}
```

The cnb-cli consumer factored the issue-side conversion into a
tiny helper because the cast appears at every issue verb call
site (commit
[`b785d35`](https://…), `crates/cnb-cli/src/commands/issue.rs:679`):

```rust
fn issue_number_i64(n: u64) -> Result<i64, CliError> {
    i64::try_from(n).map_err(|_| CliError::BadArgs(format!("issue number out of range: {n}")))
}
```

…and the assignee endpoints sit right next to that helper but
*don't* call it, because they need a `String` instead.

## Suggested fix

Pick one representation and apply it everywhere. The two
options that would each remove the entire conversion dance:

1. **Strings everywhere** (matches the current DTO fields).
   Change the method arguments to `&str` / `String` on every
   `i64`-taking method in `IssuesClient`. The DTO field stays
   as `Option<String>`. This is the smaller diff — only
   argument types change.

2. **`i64` everywhere** (matches the wire's actual semantics).
   Change `Option<String>` to `Option<i64>` on `Issue` /
   `IssueDetail` / `Pull` / `PullRequest` *and* change the two
   string-taking issue methods (`post_issue_assignees`,
   `delete_issue_assignees`) and every pull method to `i64`.
   Larger DTO surface change, but it removes the
   stringly-typed-id smell.

Either way: please make the choice **uniform across `issues`
and `pulls`**. They model analogous concepts and shouldn't
disagree on something this fundamental.

## Why we filed this separately

The conversion dance is contagious. Any downstream type that
threads issue numbers through its own layers (cnb-cli has a
`u64` from clap, an `i64` for issue verbs, a `String` for
assignee verbs, and a `String` field on the DTO) has to pick
one side at every boundary and stick with it. A single change
upstream removes a whole category of "did I cast in the right
direction here?" review chatter from every consumer.

## Severity

Annoyance — but a contagious one. No single call site is
broken; every call site is mildly inconvenient.
