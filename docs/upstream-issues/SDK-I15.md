# `list_package_tags` returns `models::Tag` (a git-tag DTO) instead of `Vec<RegistryPackageTag>`

> SDK ref: `cnb 0.2.x`
> Tracking id (consumer side): SDK-I15 — see
> [`cnb-cli` sdk-issues.md](https://…)

## TL;DR

`RegistriesClient::list_package_tags` is typed as returning
`crate::models::Tag` — a single-object DTO that models a **git
tag** (`{commit, name, target, target_type, verification}`).

The actual endpoint
`GET /{slug}/-/packages/{type}/{name}/-/tags` returns an
**array** of registry-package-tag summaries (each with `name`,
`updated_at`, etc.) — a completely different schema and
cardinality. The two types share nothing but a coincidence of
naming.

Result: typed deserialisation either fails outright (array →
struct) or, in the unlikely case the server ever wraps the
response in an object with a `name` field, silently picks up
just that one field. Either way the typed path is unusable.

## Surface area

- `cnb::registries::RegistriesClient::list_package_tags(slug, kind, name, &ListPackageTagsQuery) -> Tag`
- `cnb::models::Tag` — the **git-tag** DTO with fields like
  `commit: GitCommit`, `target_type: String`,
  `verification: …`.

The endpoint shape returns something resembling:

```json
[
  {
    "name": "v1.2.3",
    "updated_at": "2026-04-01T12:34:56Z",
    "digest": "sha256:…",
    "size": 12345
  },
  ...
]
```

…which has no overlap with the git-tag `Tag` DTO at all.

## Minimal reproduction

```rust
use cnb::registries::{RegistriesClient, ListPackageTagsQuery};

async fn list_tags(c: &RegistriesClient, slug: &str, kind: &str, name: &str) -> Result<()> {
    let q = ListPackageTagsQuery::new();
    // Method signature returns `Tag` (singular). Server returns an array.
    // Deserialisation fails.
    let _t = c.list_package_tags(slug.into(), kind.into(), name.into(), &q).await?;
    Ok(())
}
```

Wiremock variant against the canonical wire shape:

```rust
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn list_package_tags_fails_to_deserialise() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ORG/REPO/-/packages/docker/myimage/-/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "v1.0.0", "updated_at": "2026-01-01T00:00:00Z"},
            {"name": "latest",  "updated_at": "2026-01-02T00:00:00Z"}
        ])))
        .mount(&server)
        .await;

    let c = cnb::ApiClient::builder()
        .token("test")
        .base_url(server.uri().parse().unwrap())
        .build()
        .unwrap();

    let q = cnb::registries::ListPackageTagsQuery::new();
    let err = c.registries()
        .list_package_tags("ORG/REPO".into(), "docker".into(), "myimage".into(), &q)
        .await
        .unwrap_err();
    // err: invalid type: sequence, expected struct Tag
    assert!(format!("{err:?}").contains("expected struct"));
}
```

## What we did downstream (and why we'd rather not)

In the cnb-cli consumer (commit
[`b785d35`](https://…), `crates/cnb-cli/src/commands/registry.rs:374`):

```rust
async fn tag_list(ctx: &mut Context, args: PackageRefArgs) -> Result<(), CliError> {
    // 1. Issue the typed call to exercise the SDK's request path
    //    (auth, base URL, retries, tracing). We have to discard the
    //    result via `unwrap_or_default()` because it can never be
    //    correct on a successful 200.
    let _typed = {
        let q = ListPackageTagsQuery::new();
        ctx.sdk()?.registries()
            .list_package_tags(args.slug.clone(), args.kind.clone(), args.name.clone(), &q)
            .await
            .unwrap_or_default()
    };

    // 2. Re-issue the request as a raw `serde_json::Value` to actually
    //    get the array back, and render it through the normal
    //    `--json / --jq / --template / table` pipeline.
    let v = ctx.sdk_raw_get(&format!(
        "/{}/-/packages/{}/{}/-/tags",
        args.slug, args.kind, args.name
    )).await?;
    render(ctx, &args.out, &v)?;
    Ok(())
}
```

The double request is wasteful but unavoidable: we want the
typed path to share the SDK's connection pool / auth / tracing,
yet the response shape is unusable. So we pay for one
round-trip we throw away, and one we actually render.

## Suggested fix

Two related changes:

1. **Introduce a dedicated DTO**, e.g.

   ```rust
   #[derive(Debug, Clone, Deserialize, Serialize)]
   pub struct RegistryPackageTag {
       pub name: String,
       pub updated_at: Option<String>,
       pub digest: Option<String>,
       pub size: Option<i64>,
       // …whatever else the spec documents
   }
   ```

2. **Retype the method**:

   ```rust
   pub async fn list_package_tags(
       &self,
       slug: String,
       kind: String,
       name: String,
       q: &ListPackageTagsQuery,
   ) -> Result<Vec<RegistryPackageTag>, …>;
   ```

The git-tag `Tag` DTO should stay where it belongs — on
`git::GitClient` for the `/{repo}/-/git/tags/{name}` endpoint.
The naming collision is the entire root cause here.

## Severity

Blocker for the typed path. Every consumer of
`list_package_tags` has to bypass the SDK's typed surface and
go via raw JSON, exactly like SDK-I02 (`Repos4User` omits
`default_branch`) — and unlike SDK-I02 there isn't even a
"missing field" workaround that lets the typed path partially
succeed; the array-vs-struct mismatch fails 100% of the time.
