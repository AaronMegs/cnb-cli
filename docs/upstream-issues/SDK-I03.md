# `Visibility` alias rejects integer-form server responses

> SDK ref: `cnb 0.2.x` (alias `cnb-sdk` in the consumer workspace)
> Tracking id (consumer side): SDK-I03 — see
> [`cnb-cli` sdk-issues.md](https://… link to whatever public mirror you wish)

## TL;DR

`pub type Visibility = String;` (in `cnb::models`) is a strict
string alias, but several real cnb.cool deployments — including
older self-hosted instances and a fair chunk of public test
fixtures — emit `visibility_level` as an **integer** (`0` /
`10` / `20`). Any typed deserialisation of `Repos4User` (and any
other DTO that carries `visibility_level: Option<Visibility>`)
fails with:

```text
invalid type: integer `0`, expected a string
```

This blocks the typed path entirely against any server that
hasn't migrated to the canonical string form yet.

## Surface area

- `cnb::models::Visibility` (= `String`)
- `cnb::models::Repos4User.visibility_level: Option<Visibility>`
- Same field on every other DTO that exposes a visibility, e.g.
  `Repos4User`, list-shaped repo DTOs, registry visibility.

## Minimal reproduction

```rust
use cnb::ApiClient; // alias `cnb-sdk` if your bin is also `cnb`

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ApiClient::builder()
        .token("…")
        .base_url("https://api.cnb.cool".parse()?)
        .build()?;

    // Any repo on a server that still emits the int form trips this:
    //   GET /{slug} → {"visibility_level": 0, …}
    // SDK error: `invalid type: integer 0, expected a string`
    let _r = client.repositories().get_by_id("ORG/REPO").await?;
    Ok(())
}
```

A standalone wiremock reproduction (no live server needed):

```rust
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn integer_visibility_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ORG/REPO"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "path": "ORG/REPO",
            "name": "REPO",
            "visibility_level": 0  // legacy integer form, NOT "public"
        })))
        .mount(&server)
        .await;

    let client = cnb::ApiClient::builder()
        .token("test")
        .base_url(server.uri().parse().unwrap())
        .build()
        .unwrap();

    let err = client.repositories().get_by_id("ORG/REPO").await.unwrap_err();
    // err is a DeserializationError carrying:
    //   `invalid type: integer 0, expected a string`
    assert!(format!("{err:?}").contains("expected a string"));
}
```

## Observed integer mapping

| Wire integer | Canonical string |
|--------------|------------------|
| `0`          | `public`         |
| `10`         | `internal`       |
| `20`         | `private`        |

This mapping is consistent across the cnb.cool deployments we
have seen and matches the legacy GitLab-style numeric encoding
(`Public=0`, `Internal=10`, `Private=20`).

## What we did downstream (and why we'd rather not)

In the cnb-cli consumer (commit
[`b785d35`](https://… cnb-cli mirror)) we:

1. **Updated every wiremock fixture** to the canonical string
   form so our own integration tests at least exercise a
   consistent shape.
2. Added `cnb-cli::commands::repo::format_visibility()` that
   tolerates **both** forms on the *display* path. This only
   helps for the raw `Value` passthrough we use for `repo view`
   (`Context::sdk_raw_get`); it does **not** help the typed
   deserialisation, which still fails outright.
3. As a result, every typed call site that touches
   `visibility_level` is wrapped in a paired raw-`Value` GET so
   the user-visible output stays correct even when the typed
   path bombs.

(See `format_visibility` and its unit tests in
`crates/cnb-cli/src/commands/repo.rs` — both string and integer
inputs are tested.)

## Suggested fix

Either of the two would unblock us. Option (1) is preferred
because it preserves typed access on the happy path:

1. **Custom `Deserialize` for `Visibility`** that accepts both:

   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
   #[serde(rename_all = "lowercase")]
   pub enum Visibility {
       Public,
       Internal,
       Private,
   }

   impl<'de> Deserialize<'de> for Visibility {
       fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
           // accept "public" | "internal" | "private"
           // accept 0      | 10        | 20
           // anything else → custom error
           ...
       }
   }
   ```

   This round-trips back out to the canonical string form,
   which gives the OpenAPI spec the cleanest representation.

2. **Loosen the DTO** to `Option<serde_json::Value>` until the
   server converges on one wire shape. Less ideal — pushes the
   problem onto every consumer — but unblocks typed
   deserialisation immediately.

## Severity

Blocker for anyone wiring the SDK against a cnb.cool instance
that hasn't migrated to the string form. Currently forces a
pair-the-typed-call-with-a-raw-Value-call workaround on every
repo read site.
