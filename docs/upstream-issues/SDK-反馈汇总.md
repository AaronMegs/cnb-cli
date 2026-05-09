# cnb SDK（`cnb` crate）问题反馈汇总（v0.2.x）

<!-- markdownlint-disable MD024 MD031 MD060 -->

> **目标读者**：cnb SDK（[crates.io: `cnb`](https://crates.io/crates/cnb)，作者 AaronMegs）的上游维护者。
> **来源**：在 `cnb-cli` 项目（一个第三方 CLI 消费者，二进制名也叫 `cnb`，因此在工作区里把 SDK 改名为 `cnb-sdk` 引用）从手写 facade 完整迁移到 typed SDK 期间累计观察到的 19 个问题。
> **SDK 版本**：`cnb 0.2.x`（消费者 `Cargo.toml` 中：`cnb-sdk = { package = "cnb", version = "0.2", default-features = false, features = ["rustls-tls", "retry", "all-resources"] }`）。
> **锚定 commit**：`b785d35`（cnb-cli Phase 2 step 2.11，typed-SDK 迁移结束）。
> **统计**：19 项 open / 0 项 resolved。
> **附件**：英文 minimal repro 见同目录 `SDK-I03.md` / `SDK-I07.md` / `SDK-I09.md` / `SDK-I14.md` / `SDK-I15.md` / `Tier-B.md` / `Tier-C.md`。

---

## 0. 摘要表（用于 triage 和提单清单）

按上游处理顺序排列；**Tier A** 单独提 issue，**Tier B / Tier C** 分别合并成一个 issue。

| 序号 | ID       | 严重程度    | 一句话描述                                                                                  | Tier  |
|----:|----------|------------|---------------------------------------------------------------------------------------------|-------|
| 1   | SDK-I03  | **blocker**| `Visibility` 别名只接受字符串；老服务器仍发整数 `0/10/20`，typed 反序列化直接失败          | **A** |
| 2   | SDK-I07  | annoyance  | issue/pull 编号类型在 SDK 内不一致：`i64` vs `String`，跨模块也不一致                       | **A** |
| 3   | SDK-I09  | blocker-邻 | `Pull.{head,base}` 是 `Option<Value>`，每个消费者都要自写 `read_branch` helper             | **A** |
| 4   | SDK-I14  | annoyance  | HTTP 层只支持 JSON；release upload/download、build logs 三处都要起 side-car `reqwest`     | **A** |
| 5   | SDK-I15  | **blocker**| `list_package_tags` 返回单对象 `Tag`（git tag DTO），但服务端实际返回数组，typed 路径完全不可用 | **A** |
| 6   | SDK-I01  | annoyance  | `HttpInner::url()` / `base_url` 是 `pub(crate)`，无法干净构造 raw 请求                     | **B** |
| 7   | SDK-I02  | annoyance  | `Repos4User` DTO 漏掉服务端字段（如 `default_branch`），typed 调用静默丢字段              | **B** |
| 8   | SDK-I08  | annoyance  | 同一资源两份 DTO：`Pull` vs `PullRequest`，字段集互有取舍                                  | **B** |
| 9   | SDK-I11  | annoyance  | `RepoPatch` 缺 `name` / `default_branch`，原 facade 默默送达的字段现在送不出               | **B** |
| 10  | SDK-I13  | polish     | `list_forks_repos` 返回 `ListForks { forks: Option<Vec<_>> }`，破坏其它 list 的统一签名     | **B** |
| 11  | SDK-I19  | annoyance  | PR 写路径 DTO 缺 `assignees`/`labels`/`base`/`remove_source_branch`                         | **B** |
| 12  | SDK-I04  | polish     | crate 名 `cnb` 与下游同名 binary 冲突，必须用 `package =` 别名                              | **C** |
| 13  | SDK-I05  | polish     | query 结构体未加 `#[non_exhaustive]`，直接初始化是 SemVer 隐患                              | **C** |
| 14  | SDK-I06  | polish     | crate metadata 中 `repository` URL 未鉴权访问 404                                           | **C** |
| 15  | SDK-I10  | annoyance  | URL path 段无校验/转义，用户可控 ID 含 `/` 时会"路由漂移"成 404                            | **C** |
| 16  | SDK-I12  | annoyance† | `set_repo_visibility` 用 query string，与原 facade 用 body 不一致；canonical 待服务端确认  | **C** |
| 17  | SDK-I16  | annoyance† | `UpdateMembersRequest` body 形状（`access_level`）与原 facade（`role`）不一致；同样待确认  | **C** |
| 18  | SDK-I17  | annoyance  | `GetRepoContributorTrendQuery` 缺 `days` 过滤参数（服务端实际接受）                         | **C** |
| 19  | SDK-I18  | annoyance  | `pinned-repos` 只生成了 GET，没有 PUT；写半边缺失                                          | **C** |

† 标注的两项依赖服务端确认 wire shape，可能升级为 blocker。

---

## 1. 提单/合入建议（rollout plan）

为节约维护者的 triage 成本，建议按以下结构提单：

1. **Tier A · 5 个独立 issue**：每条都有独立的 minimal repro（见 §3，每个子节都是 self-contained），都有独立的修复手段。覆盖了"每个消费者都得写一遍同样 workaround、且 workaround 损失了 SDK 的核心收益（typed 路径、连接池复用、auth 转发）"的高杠杆问题。
2. **Tier B · 1 个合并 issue**（标题建议：*《cnb-cli 移植期间发现的 DTO 完整性 & 方法签名一致性问题》*）：6 个 DTO/签名级 nit，一次"DTO polish PR"可批量收掉。
3. **Tier C · 1 个 meta-issue**（标题建议：*《Polish & conventions》*）：8 个 housekeeping 项，按 6 个 subgroup 分组，方便维护者按子类批量处理。
4. **可选 patch PR**：以下 4 项是"最低争议"，作者完成 mirror URL 确认后可直接发 PR：
   - SDK-I04 · 改名/重新发布为 `cnb-sdk` 或 `cnb-client`
   - SDK-I05 · 给 query/body 结构体加 `#[non_exhaustive]`
   - SDK-I06 · 修 `repository` URL（公开访问 / 重定向到镜像）
   - SDK-I18 · 补上 `set_pinned_repos`（如果 OpenAPI spec 已声明）

---

## 2. 阅读约定

- **Surface（影响面）**：列出受影响的 SDK 类型 / 方法。
- **现象**：消费者侧观察到的具体行为。
- **Workaround**：cnb-cli 在 commit `b785d35` 上采用的规避方式，可作为复现入口（每条都标了具体文件:行号）。
- **建议修复**：按"侵入性最小→最干净"列出多个备选方案。
- **严重程度**：blocker（阻断 typed 路径）/ annoyance（可绕但成本高）/ polish（润色）。

---

## 3. Tier A · 高杠杆问题（建议各自独立提 issue）

### SDK-I03 · `Visibility` 别名仅接受字符串，老服务器发整数会反序列化失败 · **blocker**

- **Surface**：`cnb::models::Visibility = String`、`cnb::models::Repos4User.visibility_level: Option<Visibility>`，以及所有携带 `visibility_level` 字段的 DTO。
- **现象**：上游 OpenAPI spec 把 visibility 建模为字符串枚举（`"public"` / `"internal"` / `"private"`），SDK 因此用 `pub type Visibility = String;`。但部分 cnb.cool 部署和大量 wiremock fixture 仍发送整数 `0` / `10` / `20`。typed 反序列化报错：

  ```text
  invalid type: integer `0`, expected a string
  ```

- **观察到的整数→字符串映射**（与 GitLab 风格一致）：

  | wire 整数 | canonical 字符串 |
  |----------:|------------------|
  | `0`       | `public`         |
  | `10`      | `internal`       |
  | `20`      | `private`        |

- **wiremock 复现**（无需真实服务）：

  ```rust
  use serde_json::json;
  use wiremock::{Mock, MockServer, ResponseTemplate};
  use wiremock::matchers::{method, path};

  #[tokio::test]
  async fn integer_visibility_is_rejected() {
      let server = MockServer::start().await;
      Mock::given(method("GET")).and(path("/ORG/REPO"))
          .respond_with(ResponseTemplate::new(200).set_body_json(json!({
              "path": "ORG/REPO", "name": "REPO",
              "visibility_level": 0   // 老格式
          })))
          .mount(&server).await;
      let client = cnb::ApiClient::builder()
          .token("test").base_url(server.uri().parse().unwrap())
          .build().unwrap();
      let err = client.repositories().get_by_id("ORG/REPO").await.unwrap_err();
      assert!(format!("{err:?}").contains("expected a string"));
  }
  ```

- **Workaround**（cnb-cli 侧）：
  1. 把所有自己的 wiremock fixture 改回 canonical 字符串。
  2. `cnb-cli::commands::repo::format_visibility()` 在**展示层**同时容忍两种形态，但仅对 `sdk_raw_get` 拿到的 raw `Value` 有效；typed 路径仍然直接失败。
  3. 因此每个 typed 调用现场都不得不配一个 raw `Value` GET 兜底。
- **建议修复**：
  1. **首选**：给 `Visibility` 自定义 `Deserialize`，同时接受字符串与整数，输出回 canonical 字符串：

     ```rust
     #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
     #[serde(rename_all = "lowercase")]
     pub enum Visibility { Public, Internal, Private }

     impl<'de> Deserialize<'de> for Visibility {
         fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
             /* 接受 "public"|"internal"|"private" 与 0|10|20 */
         }
     }
     ```
  2. 退一步：暂时把字段类型放宽到 `Option<serde_json::Value>`，等 wire 形状统一再收紧。

---

### SDK-I07 · issue / pull 编号类型在 SDK 内部不一致 · annoyance

- **Surface**：

  | 方法 / DTO 字段                          | 参数类型 | DTO 字段类型     |
  |------------------------------------------|----------|------------------|
  | `IssuesClient::get_issue`                | `i64`    | `Option<String>` |
  | `IssuesClient::post_issue_assignees`     | `String` | `Option<String>` |
  | `IssuesClient::delete_issue_assignees`   | `String` | `Option<String>` |
  | `PullsClient::get_pull`                  | `String` | `Option<String>` |
  | `Issue` / `IssueDetail` / `Pull` / `PullRequest` 的 `number` 字段 | — | `Option<String>` |

- **现象**：同一个"数字资源 ID"，在 `issues` 内部参数是 `i64`、DTO 字段是 `String`、唯独 assignee 子接口又是 `String`；在 `pulls` 内部一致使用 `String`；跨模块再次冲突。
- **消费者侧的转换 dance**：

  ```rust
  // CLI 入参 u64 → SDK 要 i64
  let n_i64 = i64::try_from(n_input)
      .map_err(|_| "issue number out of range")?;
  let issue = client.get_issue(repo, n_i64).await?;

  // 同一 issue，assignee 接口要 String
  client.post_issue_assignees(repo, n_input.to_string(), &/*…*/).await?;

  // pulls 又是另一种：u64 → String
  let pr = client.get_pull(repo, n_input.to_string()).await?;

  // DTO 字段是 Option<String>，展示时还要 parse 回数字
  let n_for_display = issue.number.as_deref()
      .and_then(|s| s.parse::<i64>().ok())
      .map(|n| n.to_string())
      .unwrap_or_else(|| "?".into());
  ```

- **Workaround**：cnb-cli 把 issue 侧的 `i64` 转换抽成 helper（`crates/cnb-cli/src/commands/issue.rs:679` 的 `issue_number_i64`），但旁边的 assignee 接口因为类型不同就用不上这个 helper。
- **建议修复**：选一种统一应用：
  1. **全部用 String**（贴合现有 DTO）：把所有 `i64` 入参方法改成 `&str` / `String`，DTO 不动。
  2. **全部用 i64**（贴合"数字 ID"语义）：`Option<String>` 改成 `Option<i64>`，所有 `String` 入参方法改成 `i64`。
  - 不论哪种：**必须在 `issues` 与 `pulls` 之间统一**。

---

### SDK-I09 · `Pull.{head,base}` 是 `Option<serde_json::Value>`，每个消费者都得自写 `read_branch` · blocker-邻

- **Surface**：`cnb::models::Pull.head` / `Pull.base` / `PullRequest.head` / `PullRequest.base` 全部 `Option<serde_json::Value>`。
- **现象**：PR 最常用的两个字段（head / base 分支名）没有类型化。OpenAPI spec 没钉死 schema，SDK 因此保守拒绝下断言，但消费者**仍然必须**取到分支名才能渲染。线上至少观察到 4 种 wire 形态：

  ```json
  // shape 1（当前默认）
  {"head": {"branch": "feat/x", "commit_id": "abc1234"}}
  // shape 2
  {"head": {"ref": "refs/heads/feat/x"}}
  // shape 3
  {"head": {"name": "feat/x"}}
  // shape 4（legacy 顶层兄弟字段）
  {"source_branch": "feat/x", "target_branch": "main"}
  ```

  shape 4 尤其麻烦：`head: Option<Value>` 即使存在也拿不到这两个顶层字段，必须并联 raw GET 才能补回来。

- **Workaround**：cnb-cli 抽出 `read_branch` helper（`crates/cnb-cli/src/commands/pr.rs:371`），按优先级依次尝试 `branch` / `ref` / `name`，然后 fallback 到顶层兄弟字段。30 行代码，5 个单测覆盖所有形态。每个 PR UI 都得复刻一遍。
- **建议修复**：spec 钉死 canonical 形态（建议 `{branch, commit_id}`，与当前默认部署一致），SDK 暴露真实 DTO：

  ```rust
  #[derive(Debug, Clone, Deserialize, Serialize)]
  pub struct PullRef {
      pub branch: Option<String>,
      pub commit_id: Option<String>,
  }
  // Pull / PullRequest:
  pub head: Option<PullRef>,
  pub base: Option<PullRef>,
  ```

  消费者代码立刻塌成一行：

  ```rust
  let head = pr.head.as_ref().and_then(|r| r.branch.as_deref()).unwrap_or_default();
  ```

  对 legacy 顶层 `source_branch` / `target_branch`，可加同名兄弟字段并文档化为"仅在 head/base 缺失时填充"；或在 `branch` 上加 `#[serde(alias = "ref", alias = "name")]` 兼容历史 key。

---

### SDK-I14 · 缺少非 JSON transport，bytes 端点必须起 side-car `reqwest::Client` · annoyance（三处共享，影响放大）

- **Surface**：`cnb::http::HttpInner::{execute, execute_with_body}` 两条腿都强 JSON：
  - `execute::<T>(method, url)` 用 `serde_json::from_slice` 解码响应。
  - `execute_with_body::<T, B: Serialize>(method, url, body)` 用 `req.json(b)` 设置 `Content-Type: application/json`。
  - 没有 `reqwest::Body` 入口、没有 raw bytes 出口、底层 `reqwest::Client` 也未 `pub` 暴露。
- **三个真实受影响 flow**：

  1. **release asset 上传 phase 2**（`PUT <pre-signed url>` 文件流）：phase 1 是 JSON 走 SDK，phase 2 完全没法走 SDK。cnb-cli 的 workaround（`crates/cnb-cli/src/commands/release.rs:479`）：

     ```rust
     let file = File::open(path).await?;
     let stream = ReaderStream::new(file);
     let body = reqwest::Body::wrap_stream(stream);
     let put_resp = reqwest::Client::new()                 // ← side-car
         .put(&upload_url)
         .header(reqwest::header::CONTENT_LENGTH, size)
         .body(body).send().await?;
     ```

  2. **release asset 下载**（`GET /{repo}/-/releases/download/{tag}/{file}`，响应是 302 + 文件 bytes）：SDK 的 `get_releases_asset` 把响应类型写成 `serde_json::Value`，遇到非 JSON 直接 `expected value at line 1 column 1`。
  3. **runner 日志下载**（`GET /{repo}/-/build/runner/download/log/{pid}`，响应 `text/plain`）：`build_runner_download_log` 同样 JSON 解码，必败。

- **side-car 的代价**（每次调用都重复一遍）：连接池不复用、retry 不复用、auth header 自己重新拼、tracing 不进 SDK 的 span、连 SDK 已解析过的 token 都拿不到（必须再读一次 `CNB_TOKEN`）。三个不相关动词共享同一个 workaround，使得"统一修一次"价值远高于单点修复。
- **建议修复**（三选一即可解锁全部三个 flow）：
  1. **方案 A · 暴露底层 `reqwest::Client`**（最小补丁，与 SDK-I01 天然配套）：

     ```rust
     impl HttpInner {
         pub fn reqwest_client(&self) -> &reqwest::Client { &self.client }
         pub fn base_url(&self) -> &Url { &self.base_url }
         pub fn url(&self, path: &str) -> Result<Url, …> { self.base_url.join(path).map_err(|e| …) }
     }
     ```
  2. **方案 B · 扩展 `execute_with_body` 接受任意 `reqwest::Body`** + 新增 `execute_bytes(...) -> bytes::Bytes`（流式响应）。
  3. **方案 C · 把三个 bytes 端点都建模成 first-class typed 方法**：`upload_release_asset(path)` / `download_release_asset(...) -> Bytes` / `download_runner_log(...) -> String`。最干净，补丁也最大。

  推荐 A 先合，C 作为后续打磨。

---

### SDK-I15 · `list_package_tags` 返回单对象 `Tag`（git tag DTO），typed 路径完全不可用 · **blocker**

- **Surface**：`RegistriesClient::list_package_tags(...) -> cnb::models::Tag`。
- **现象**：端点 `GET /{slug}/-/packages/{type}/{name}/-/tags` 实际返回**数组**：

  ```json
  [
    {"name": "v1.2.3", "updated_at": "2026-04-01T12:34:56Z", "digest": "sha256:…", "size": 12345},
    {"name": "latest", "updated_at": "2026-04-02T00:00:00Z", "digest": "sha256:…", "size": 23456}
  ]
  ```

  但 SDK 把它类型化成 `models::Tag` —— 一个 git tag DTO（`{commit, name, target, target_type, verification}`），shape 和命名都对不上。typed 调用 100% 失败：`invalid type: sequence, expected struct Tag`。
- **wiremock 复现**：

  ```rust
  let server = MockServer::start().await;
  Mock::given(method("GET"))
      .and(path("/ORG/REPO/-/packages/docker/myimage/-/tags"))
      .respond_with(ResponseTemplate::new(200).set_body_json(json!([
          {"name":"v1.0.0", "updated_at":"2026-01-01T00:00:00Z"},
          {"name":"latest", "updated_at":"2026-01-02T00:00:00Z"}
      ])))
      .mount(&server).await;
  let q = ListPackageTagsQuery::new();
  let err = client.registries()
      .list_package_tags("ORG/REPO".into(),"docker".into(),"myimage".into(),&q).await.unwrap_err();
  assert!(format!("{err:?}").contains("expected struct"));
  ```

- **Workaround**（`crates/cnb-cli/src/commands/registry.rs:374`）：先发一次 typed 调用、`unwrap_or_default()` 丢弃响应（仅为了过 SDK 的 auth/retry/tracing 通道），再用 `Context::sdk_raw_get` 重新发一次取真实数组。**双请求**，浪费一次 round-trip。
- **建议修复**：
  1. 引入专用 DTO：

     ```rust
     #[derive(Debug, Clone, Deserialize, Serialize)]
     pub struct RegistryPackageTag {
         pub name: String,
         pub updated_at: Option<String>,
         pub digest: Option<String>,
         pub size: Option<i64>,
     }
     ```
  2. 把方法签名改回 `Result<Vec<RegistryPackageTag>, …>`。git tag 的 `Tag` DTO 留在 `git::GitClient` 上即可，命名冲突是这条问题的全部根因。

---

## 4. Tier B · DTO 完整性 & 方法签名一致性合并稿（建议合并成一个 issue）

> 标题建议：*《cnb-cli 移植期间发现的 DTO 完整性 & 方法签名一致性问题》*。
> 6 个子项均为加性修改、SemVer 友好；建议作为一个 "DTO polish PR" 落地。

### SDK-I01 · `HttpInner::url()` 与 `base_url` 是 `pub(crate)`

- **Surface**：`cnb::http::HttpInner`。
- **现象**：`HttpInner::url(path)` 与 `base_url: Arc<Url>` 字段都是 `pub(crate)`。`HttpInner::execute::<T>(method, url)` 是 `pub` 但需要外部传入完整 `Url`，而**没人能在 crate 外正确构造它**（base URL 优先级、trailing slash、percent-encoding 全是 SDK 内部规则）。
- **Workaround**：`Context::sdk_raw_get(path)` 在 CLI 里重新计算 base URL（`CNB_API_BASE` > SDK default），用 `Url::parse().join()` 拼出最终 URL，再丢给 `HttpInner::execute::<Value>`。能跑，但 CLI 必须知道两件 SDK 已经知道的事：base URL 和 join 规则。
- **建议修复**（任选其一）：
  1. 把 `HttpInner::url(path)` 设为 `pub`（最小补丁）。
  2. 加 `pub fn ApiClient::base_url(&self) -> &Url` + 文档化 `Url::join` 契约。
  3. 加便捷方法 `HttpInner::get_json<T>(path)`，封装 `url()` + `execute::<T>()`。
- **关联**：与 SDK-I14 的"暴露 `reqwest::Client`"诉求天然一对。

### SDK-I02 · `Repos4User` DTO 漏掉服务端字段（如 `default_branch`）

- **Surface**：`cnb::models::Repos4User`，`repositories::get_by_id` 返回。
- **现象**：服务端 `GET /{repo}` 返回的 `default_branch` 等字段在 DTO 上没有；typed 调用静默丢字段，`--json` 输出对用户撒谎。
- **Workaround**：`cnb repo view` 双发——一次 typed（捕获 schema 回归），一次 raw `Value`（保留全字段）。共享 SDK 连接池，单对象 GET 成本可接受；但若发生在 list 端点上代价就大了。
- **建议修复**：
  1. 给 `Repos4User` 补上 `default_branch` 等已确认字段。
  2. 长期方案：在 generator 里默认加 `#[serde(flatten)] extra: HashMap<String, Value>` catch-all，让未文档化字段默认存活。

### SDK-I08 · 同一资源两份 DTO：`Pull` vs `PullRequest`

- **Surface**：`PullsClient::get_pull() -> Pull` vs `list_pulls() -> Vec<PullRequest>`。
- **现象**：字段集互有取舍：

  | 字段             | `Pull`              | `PullRequest`               |
  |------------------|---------------------|-----------------------------|
  | `labels`         | `Vec<LabelInfo>`    | `Vec<Label>`                |
  | `comment_count`  | 缺                  | `Option<i64>`               |
  | `review_count`   | 缺                  | `Option<i64>`               |
  | `repo`           | 缺                  | `Option<serde_json::Value>` |
  | `created_at`     | 缺                  | `Option<String>`            |
  | `last_acted_at`  | 缺                  | `Option<String>`            |
  | `reviewers`      | `Vec<PullReviewer>` | 缺                          |
  | `updated_at`     | `Option<String>`    | `Option<String>`            |

  对应 spec 的 "detail vs list item" 区分，理论合理实务痛苦：想复用 `list` 与 `view` 的渲染代码，要么取最小公约数，要么写双 match。
- **Workaround**：渲染前都转 `serde_json::Value`，丢失 typed 路径收益换共用渲染。
- **建议修复**：
  1. **首选**：单一 `Pull`，把 list 才有的统计字段塞进 `Option<PullStats>` 子结构。
  2. **替代**：用 `#[serde(flatten)]` 抽出共有部分，让消费者可以解构共同视图。
- **关联**：`Issue` vs `IssueDetail` 完全同形，建议同一波修。

### SDK-I11 · `RepoPatch` 是历史 `cnb repo edit` 接受字段的真子集

- **Surface**：`cnb::models::RepoPatch`（`RepositoriesClient::update_repo`）。
- **现象**：typed PATCH 体只有 `description` / `license` / `site` / `topics`；缺 `name` 与 `default_branch`，原 facade 是有的。我们没有证据判断服务端到底是默默接受这两个字段、还是要走另一个端点（`set-default-branch` / `rename`，SDK 也未建模）。
- **Workaround**：`cnb repo edit` 把 `--name` 和 `--default-branch` 直接 `BadArgs`（exit 3）拒绝，只保留 `--description` —— 不静默丢，比原 facade 默送更诚实。
- **建议修复**：
  1. 若服务端真的支持，给 `RepoPatch` 补上这两个字段。
  2. 否则建模专用方法（`rename_repo` / `set_default_branch`），并在文档明示。

### SDK-I13 · `list_forks_repos` 返回 wrapper 而非 `Vec`

- **Surface**：`RepositoriesClient::list_forks_repos(...) -> ListForks { fork_tree_count, forks: Option<Vec<Forks>> }`。
- **现象**：所有 list 方法都返回 `Vec<T>`，唯独这个套了 wrapper。不是 bug，但每个消费者都得 `.forks.unwrap_or_default()`，破坏统一模式。
- **Workaround**：`cnb repo fork` 解出 `.forks` 默认空 vec，`--json` 输出保持数组。
- **建议修复**：
  1. 重命名为 `get_fork_summary_repos` 让 wrapper 显式化；或
  2. 增加同胞方法 `list_forks_repos_flat -> Vec<Forks>`。

### SDK-I19 · PR 写路径 DTO 缺关键字段

- **Surface**：
  - `PullCreationForm` 缺 `assignees` / `labels`。
  - `PatchPullRequest` 缺 `base`（无法 retarget PR）。
  - `MergePullRequest` 缺 `remove_source_branch`。
  - 顺带：merge body 用 `merge_style` 而原 facade 用 `merge_method`，wire 实际名字未被独立验证。
- **现象**：原 facade 默送这些字段（不论服务端是否处理），typed DTO 严格 schema 直接丢失能力。
- **Workaround**：CLI 把 `pr create --assignee/--label`、`pr edit --base`、`pr merge --delete-branch` 全部 `BadArgs` 拒绝，提示组合替代（`pr create` + `pr assign --add` / `pr label --add`；merge 后单独删源分支）。
- **建议修复**：
  1. `PullCreationForm` 加 `assignees: Option<Vec<String>>` / `labels: Option<Vec<String>>`。
  2. `PatchPullRequest` 加 `base: Option<String>`；若服务端真不支持 retarget，请在方法 doc 上明示。
  3. `MergePullRequest` 加 `remove_source_branch: Option<bool>`；同时确认 `merge_style` vs `merge_method` 的 canonical 名。

### Tier B 建议合入顺序（如果走单 PR）

1. SDK-I01（一个关键字翻 `pub`）
2. SDK-I02（加一个字段）
3. SDK-I19（3 个 DTO 共加 4 个 `Option` 字段，全加性）
4. SDK-I11（加字段或加方法）
5. SDK-I13（重命名或加同胞方法）
6. SDK-I08（最大改面，单独评审）

---

## 5. Tier C · housekeeping meta-issue（建议合并成一个 issue，按 subgroup 分组）

> 标题建议：*《Polish & conventions》*。
> 各子项分到 6 个 subgroup，便于维护者按子类批量处理。

### Subgroup 1 · 发布元数据

#### SDK-I04 · crate 名 `cnb` 与下游同名 binary 冲突

- **现象**：消费者的二进制也叫 `cnb`，直接 `cnb = "0.2"` 在同一工作区里产生包名歧义；所有 `-p cnb` 都需要消歧（`-p cnb@0.4.0-alpha.1` 或 `--manifest-path`）。
- **Workaround**：`Cargo.toml` 用 `cnb-sdk = { package = "cnb", version = "0.2", … }` 重命名，代码统一 `cnb_sdk::…`。能跑，但每个 Cargo.toml、每行 `use` 都背着 rename 噪音。
- **建议修复**：在 crates.io 上重新发布为 `cnb-sdk` 或 `cnb-client`；现 crate 可作为薄壳过渡期 re-export。

#### SDK-I06 · crate metadata 中 `repository` URL 未鉴权访问 404

- **现象**：crate metadata 里 `repository = "https://cnb.cool/aodoo/tools/rust-cnb.git"`，对应 web URL 未登录访问返回 "页面不存在或访问权限不足"。docs.rs 兜住了大部分内容，但顺着 crates.io "Repository" 链接的人会落到死页面。
- **Workaround**：直接读 `docs.rs/cnb/0.2.1/cnb/`。
- **建议修复**：仓库公开读权限，或把 `repository` 字段改为可浏览的镜像（GitHub / source.gc 等）。
- **副作用**：本文档以及 `docs/upstream-issues/*.md` 中所有 `https://…` 占位符都是在等这个 URL —— 一旦确认，单次 `sed` 替换。

### Subgroup 2 · 生成代码约定

#### SDK-I05 · query 结构体未加 `#[non_exhaustive]`

- **现象**：`GetReposQuery` / `GetReposByUserNameQuery` / `GetGroupSubReposQuery` 等 query 结构体是 `#[derive(Default)]` + 公开字段、无 `#[non_exhaustive]`。如果未来 minor 版加新可选字段，使用 positional struct init 或 pattern match 的消费者会编译失败 —— SemVer 隐患。
- **Workaround**：cnb-cli 全部走 builder 链（`GetReposQuery::new().page(…).page_size(…)`），避免 `GetReposQuery { page: ..., ..Default::default() }`。约定写在 contributor notes 里。
- **建议修复**：所有 query / body 结构体加 `#[non_exhaustive]`；或字段 `pub(crate)` 化、强制 builder-only。

### Subgroup 3 · 防御性默认

#### SDK-I10 · URL path 段无校验 / 无转义

- **现象**：方法用 `format!("/foo/{}/bar", arg).join_onto(base)` 拼 URL；`arg` 含 `/` 时会被 `Url::join` 当作 path 分隔符，而不是被 percent-encode 成单个 segment。SDK 不做校验也不做编码。例：label 名字含 `/` 时（如 `evil/../leak`）静默路由到完全不同的端点，得到 5 层栈深的 "endpoint not found" 而非干净的 validation 错误。
- **不是可利用的安全漏洞**（服务端会 404 garbage path），但产生混乱报错。
- **Workaround**：cnb-cli 在调入 SDK 前自己校验，例如 `cnb-cli::commands::label::ensure_label_name_safe()` 镜像了原 `cnb-api::services::labels::ensure_no_slash()` 的逻辑，违例直接 `CliError::BadArgs`。
- **建议修复**：
  1. SDK 在拼 URL 时 percent-encode 每个 path segment（安全默认，`evil/../leak` 变成单字面 segment）；或
  2. SDK 拒绝包含 `/` 的 segment 并返回 typed 错误，方法文档里写明约束。
  - 二者择一，目的是把规则集中在 SDK，避免下游各自定义。

### Subgroup 4 · spec / 服务端 wire 形状对齐（待服务端确认）

#### SDK-I12 · `set_repo_visibility` 用 query string 而非 body

- **现象**：SDK 发出 `POST /{repo}/-/settings/set_visibility?visibility=public`，原 facade 是 JSON body `{"visibility_level": 0}`。两者不可能都对，且我们没有任何一边的集成证据（legacy 里也没有 wiremock 测试）。
- **Workaround**：`repo set-visibility` 集成测试按 SDK 的 query 形态写。如果真服务器拒，本条升级为 blocker，CLI 再用 `Context::sdk_raw_json(POST, path, body)` 走 body 形态。
- **建议修复**：找服务端确认 canonical 形状，文档化在方法上。

#### SDK-I16 · `UpdateMembersRequest` 与原 facade body 形状不一致

- **现象**：typed body 是 `{access_level, is_outside_collaborator}`，原 facade 是 POST `{username, role}` / PUT `{role}`。SDK 字段名 `access_level` 与 facade 读响应时用的 `role` key 也不一致，展示侧也得重学一遍。同样，没有 wire 实证哪个被服务端接受。
- **Workaround**：CLI 的 `org member add/edit` 把 `--role <value>` 直接搬到 `UpdateMembersRequest.access_level`，`is_outside_collaborator: None`；展示侧改读 `access_level`，原 facade 对 `role` key 的容忍丢掉了。
- **建议修复**：
  1. 服务端确认后文档化 `access_level` 为 canonical，下线 `role`；或
  2. 在 `access_level` 上加 `#[serde(alias = "role")]` 兼容历史 key（请求与响应都生效）。

### Subgroup 5 · query 完整性

#### SDK-I17 · `GetRepoContributorTrendQuery` 缺 `days` 过滤

- **现象**：服务端 `GET /{slug}/-/contributor/trend` 接受 `?days=N`，原 facade + CLI 自 M4 起就有 `--days` flag。SDK 的 query 结构体只暴露 `limit`、`exclude_external_users`，typed 路径无法表达 `days`。`limit` 是结果条数上限、不是时间窗，语义不能替代。
- **Workaround**：`cnb repo contributors` 在用户传 `--days` 时改走 `Context::sdk_raw_get`；不传时仍走 typed 路径。
- **建议修复**：给 `GetRepoContributorTrendQuery` 加 `days: Option<i64>` 与对应 builder 方法；如 OpenAPI spec 里没写明，顺手作为 spec 缺口报上去。

### Subgroup 6 · 缺失动词

#### SDK-I18 · `pinned-repos` 只有 GET，没 PUT

- **现象**：`RepositoriesClient` 暴露 `get_pinned_repo_by_group`（按 group GET）+ `get_pinned_repo_by_id`（按 user GET），但**没有** `PUT /{slug}/-/pinned-repos`（替换 pinned 集合）。`cnb repo pin` / `unpin` 需要"读 → 计算新集合 → 写回"，SDK 只给了读。
- **Workaround**：CLI 用 typed GET + `Context::sdk_raw_json(PUT, path, body)`。`sdk_raw_json` 是为这个 case 专门加的工具，会经过 SDK 的 `HttpInner::execute_with_body`，仍然共享 retry / auth / tracing。
- **建议修复**：从 spec 生成 `set_pinned_repos(slug, body: &SetPinnedRepos)`，body 形状 `{repos: Vec<String>}`，返回 `serde_json::Value`（或 spec 文档化的 ack DTO）。
- **优先级**：如果 OpenAPI spec 已经声明这个端点，是 4 个 patch-ready 项中**最直接**的 PR。

### Tier C 处理建议

- **可立即合并**：SDK-I04、SDK-I05、SDK-I06、SDK-I18 都是机械变更，patch 已 ready。
- **需服务端确认**：SDK-I12、SDK-I16，等服务端 yes/no 后 SDK 才能收敛。
- **其余**：SDK-I10、SDK-I17 可在 polish point release 一起处理。

---

## 附录 A · 锚点 commit 中可直接定位的 workaround 文件

锚点 commit：`b785d35`（cnb-cli Phase 2 step 2.11）。每条 workaround 都能在以下文件里找到具体实现：

| 文件                                                | 涉及问题                                          |
|-----------------------------------------------------|---------------------------------------------------|
| `Cargo.toml`                                        | SDK-I04（`cnb-sdk = { package = "cnb", … }`）     |
| `crates/cnb-cli/src/context.rs`                     | SDK-I01（`sdk_raw_get`、`sdk_raw_json`）          |
| `crates/cnb-cli/src/commands/repo.rs`               | SDK-I02（双 GET）、SDK-I03（`format_visibility`）、SDK-I11（`--name`/`--default-branch` 拒绝）、SDK-I13（fork 解 wrapper）、SDK-I17（`--days` 走 raw GET）、SDK-I18（pin/unpin 走 `sdk_raw_json`）|
| `crates/cnb-cli/src/commands/issue.rs`              | SDK-I07（`issue_number_i64`、assignee 用 String） |
| `crates/cnb-cli/src/commands/pr.rs`                 | SDK-I07（`get_pull` 入参 `to_string`）、SDK-I08（`Value` 渲染）、SDK-I09（`read_branch`）、SDK-I19（4 flag 拒绝）|
| `crates/cnb-cli/src/commands/registry.rs`           | SDK-I15（双发：typed + raw）                       |
| `crates/cnb-cli/src/commands/release.rs`            | SDK-I14（upload phase 2 / asset download 用 side-car reqwest）|
| `crates/cnb-cli/src/commands/build.rs`              | SDK-I14（runner log download 用 side-car reqwest）|
| `crates/cnb-cli/src/commands/label.rs`              | SDK-I10（`ensure_label_name_safe`）                |
| `crates/cnb-cli/src/commands/org.rs`                | SDK-I16（`UpdateMembersRequest.access_level`）     |
| `crates/cnb/tests/m2_repo.rs`                       | SDK-I12（query-string `set_visibility` 的 wiremock）|

---

## 附录 B · cnb-cli 侧采纳本 SDK 后产生的 triage 规则

为不打断 cnb-cli 的迁移节奏，移植期间凡是踩到 SDK 痛点都先记入 `docs/sdk-issues.md`，不就地修 SDK。目的：

1. 每个 CLI commit 聚焦在一个命令的移植上。
2. Phase 2 完成时一次性给出 well-written 的上游 issue / patch（即本文档）。
3. 给后续读者一份按时间排列的"我们在哪儿吞了哪种怪味"。

如果某条问题彻底阻断了某次移植，会就地升级为 **blocker** 并立即上报，而非等到 Phase 2 结束。

---

## 附录 C · 待外部决定的事项

1. **cnb-cli mirror URL**：本文档与同目录英文 minimal repro 中的所有 `https://…` 占位符都需要替换成 cnb-cli 的公开镜像 URL（与 SDK-I06 等价）。一旦确认即可批量 sed。
2. **SDK-I12 / SDK-I16 服务端 wire**：等服务端团队确认 canonical 形状后，SDK 才能定稿；在那之前，cnb-cli 与 SDK 走相同的 wire 形态以保持一致。
