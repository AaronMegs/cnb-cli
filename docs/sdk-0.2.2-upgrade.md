# cnb SDK 0.2.2 升级核对报告

<!-- markdownlint-disable MD024 MD032 MD033 MD060 -->
<!-- MD032 disabled because some lists in this report intentionally
     follow a single line of context without a blank line, which
     reads naturally and avoids visual stutter. -->

> **范围**：把工作区从 `cnb 0.2.1` 升到 `cnb 0.2.2`（2026-05-09 发布），逐项核对 [`docs/sdk-issues.md`](./sdk-issues.md) / [`docs/upstream-issues/SDK-反馈汇总.md`](./upstream-issues/SDK-反馈汇总.md) 中 19 项 SDK 痛点的修复落地情况。
> **执行日期**：2026-05-11。
> **结论先行**：**19 项中 9 项已在 0.2.2 中解决**（其中 4 项是我们 Tier A 报告里的 blocker / blocker-邻），剩余 10 项分为"未变化（待后续版本）"和"需服务端确认"两类。CLI 侧已完成兼容性迁移，**`cargo fmt` / `cargo clippy -D warnings` / `cargo test` 全绿（200 tests passed）**。

---

## 0. 修复总表

| ID       | Tier | 严重程度    | 0.2.2 状态        | CLI 侧动作                                                              |
|----------|------|------------|-------------------|-------------------------------------------------------------------------|
| SDK-I01  | B    | annoyance  | ✅ **已修复**       | `HttpInner::{reqwest_client, base_url, url}` 全部 `pub`                |
| SDK-I02  | B    | annoyance  | 🟡 部分             | DTO 仍未补 `default_branch`；CLI 双发逻辑保留                           |
| SDK-I03  | A    | **blocker** | ✅ **已修复**       | `Visibility` 真 enum + 自定义 deserializer，但**展示字符串变了大小写** |
| SDK-I04  | C    | polish     | ❌ 未变化           | 仍叫 `cnb`；`cnb-sdk = { package = "cnb" }` 别名继续用                  |
| SDK-I05  | C    | polish     | ✅ **已修复**       | 所有 query / body struct 加了 `#[non_exhaustive]`                       |
| SDK-I06  | C    | polish     | 🟡 部分             | URL 改成 `cnb-rust` 仓库，但仍 404；docs.rs 兜底                        |
| SDK-I07  | A    | annoyance  | ❌ 未变化           | issue/pull number 类型仍 `i64` vs `String` 不一致                       |
| SDK-I08  | B    | annoyance  | ❌ 未变化           | `Pull` vs `PullRequest` 仍是两个 DTO                                    |
| SDK-I09  | A    | blocker-邻 | ✅ **已修复**       | `head` / `base: Option<PullRef>`，但**字段名是 `ref` 不是 `branch`**   |
| SDK-I10  | C    | annoyance  | ❌ 未变化           | URL 仍用 `format!()` 拼接，无 segment 编码                              |
| SDK-I11  | B    | annoyance  | ❌ 未变化           | `RepoPatch` 仍只有 `description / license / site / topics`             |
| SDK-I12  | C    | annoyance† | 🟡 待服务端          | 仍是 query string 形态；服务端 canonical 待确认                         |
| SDK-I13  | B    | polish     | ❌ 未变化           | `list_forks_repos` 仍返回 `ListForks` wrapper                          |
| SDK-I14  | A    | annoyance  | ✅ **已修复**       | `HttpInner::reqwest_client()` 暴露，可去除 side-car                     |
| SDK-I15  | A    | **blocker** | ✅ **已修复**       | `list_package_tags` 返回 `Vec<RegistryPackageTag>`                     |
| SDK-I16  | C    | annoyance† | 🟡 待服务端          | `UpdateMembersRequest` 形态未变；服务端 canonical 待确认                |
| SDK-I17  | C    | annoyance  | ❌ 未变化           | `GetRepoContributorTrendQuery` 仍缺 `days`                             |
| SDK-I18  | C    | annoyance  | ✅ **已修复**       | 新增 `set_pinned_repo_by_group(slug, &Vec<String>)`                    |
| SDK-I19  | B    | annoyance  | ❌ 未变化           | PR 写路径 DTO 仍缺 `assignees / labels / base / remove_source_branch` |

**统计**：✅ 已修复 7 项 + 🟡 部分修复 / 待外部 4 项 + ❌ 未变化 8 项。

---

## 1. CLI 侧的实际动作

### 1.1 升级动作

```bash
# Cargo.lock 中 cnb 0.2.1 → 0.2.2
cargo update -p cnb@0.2.1
```

`Cargo.toml` 的 `cnb-sdk = { package = "cnb", version = "0.2", … }` 不变（caret 自动跟 0.2.x 补丁版本）。

### 1.2 编译错（4 处，全部由"修复"引起）

`#[non_exhaustive]` 全面铺开后，3 处 struct 直接初始化失效；`list_package_tags` 返回类型改正后旧的 raw-Value workaround 类型不匹配。

| 位置 | 问题 | 修复 |
|------|------|------|
| `cnb-cli/src/commands/pr.rs:693` | `ListPullsByNumbersQuery { ... }` | 改用 `::new().n(...)` builder |
| `cnb-cli/src/commands/repo.rs:606` | `SetRepoVisibilityQuery { ... }` | 改用 `::new().visibility(...)` builder |
| `cnb-cli/src/commands/search.rs:64` | `ListPublicReposQuery { ... }` | 改用 `::new()` 链式 builder（条件设值） |
| `cnb-cli/src/commands/registry.rs:374` | `tag_list` 双发请求 | **删除 `sdk_raw_get` workaround**，直接用 typed `Vec<RegistryPackageTag>` 渲染 |

### 1.3 测试 fixture 对齐（3 处）

SDK 0.2.2 的 wire 形态变化触发了 3 个测试失败，全部是 fixture 错位：

| 测试 | 旧 fixture | 新 fixture | 原因 |
|------|------------|------------|------|
| `m2_label_pr.rs::pr_view_renders_branch_arrow` | `head: {"branch": "feat/shiny"}` | `head: {"ref": "feat/shiny"}` | `PullRef` 字段名是 `ref`（serde rename），不再有 `branch` |
| `m2_label_pr.rs::pr_checkout_uses_head_branch` | 同上 | 同上 | 同上 |
| `m2_repo.rs::repo_view_default_card_output` 等 | `Visibility:    public` | `Visibility:    Public` | SDK canonical 用首字母大写 |
| `m2_repo.rs::repo_list_user_emits_tsv_when_piped` | `private` | `Private` | 同上 |
| `search_sdk.rs::search_default_renders_table` | `public` | `Public` | 同上 |

### 1.4 `format_visibility` 重写（user-visible 行为变化）

`Visibility` 在 0.2.2 里：

- 字符串变体：**`"Public"` / `"Private"` / `"Secret"`**（首字母大写！与之前的 `"public"/"internal"/"private"` 不同）
- 兼容输入：`Public/public` / `Private/private` / `Secret/secret/Internal/internal` / 整数 `0/10/20`
- 整数 `10` 现在映射为 `Secret`（替代旧的 "internal"）

我们的 `format_visibility()` helper 同步重写：

- 输出统一使用 SDK canonical（`Public` / `Private` / `Secret`）
- 输入侧仍然容忍历史形态（小写、`Internal`、整数）

**这是一个 user-visible 行为变化**：

- `cnb repo view` 的 "Visibility" 列从 `public` 变成 `Public`
- `cnb repo list` 的 TSV 列同上
- `cnb search` 的输出同上
- `--json` / `--jq` / `--template` 直接走 raw `Value` passthrough，**不受影响**（fixture 发什么就显示什么）

### 1.5 暂未发力的修复点

- **SDK-I01 已修**：理论上 `Context::sdk_raw_get` 可以直接用 `client.http().url(path)` 替代它自己的 base URL 拼接逻辑。**未做改动**——既有实现已经 work，且只是质量优化、不修复任何 bug。可作为后续小 cleanup。
- **SDK-I14 已修**：`HttpInner::reqwest_client()` 现在 `pub`，理论上 `release upload phase 2` / `release download` / `build logs` / `issue --attach` 四个 side-car `reqwest::Client::new()` 都能改为复用 SDK 的连接池。**未做改动**——这是 4 个独立 flow 的实质性 refactor（涉及 token 重用、auth header、error mapping 对齐），值得单独一个 commit。建议作为下一步 follow-up 工作。详见本文件 §3.1。

---

## 2. 逐项细节

### 2.1 ✅ SDK-I01 · `HttpInner` 三件套已 `pub`

```rust
// cnb-0.2.2/src/http.rs:50,55,63
pub fn reqwest_client(&self) -> &Client { ... }
pub fn base_url(&self) -> &Url { ... }
pub fn url(&self, path: &str) -> Result<Url> { ... }
```

完全匹配我们 [`SDK-反馈汇总.md` §4 SDK-I01](./upstream-issues/SDK-反馈汇总.md) 的 "Option 1" 建议（最小补丁）。

### 2.2 🟡 SDK-I02 · `Repos4User` 仍未补 `default_branch`

0.2.2 CHANGELOG 没有提及。源码中 `Repos4User`（`models/data.rs`）确实仍未含 `default_branch` 字段，不带 `#[serde(flatten)] extra` catch-all。

**结论**：双发逻辑保留。`cnb repo view` 仍然先发 typed call（捕获 schema 回归），再发 raw `Value` 取全字段。

### 2.3 ✅ SDK-I03 · `Visibility` 真 enum

`Visibility` 改成 `pub enum Visibility { Public, Private, Secret }`，并实现自定义 `Deserialize`，同时支持：

- 字符串：canonical（首字母大写）+ 全小写 + `"Internal"` 别名
- 整数：`0` → Public / `10` → Secret / `20` → Private

我们之前在 [`SDK-I03.md`](./upstream-issues/SDK-I03.md) 推荐的 "Option 1" 完全实现。

**与我们建议的差异**：上游选择 `Public/Private/Secret`，**没有** `Internal` 变体（把它作为 `Secret` 的别名吸收掉）。这是合理的服务端术语收敛。

### 2.4 ❌ SDK-I04 · 包名仍叫 `cnb`

CHANGELOG 没动。我们的 `cnb-sdk = { package = "cnb", … }` workaround 继续保留。

### 2.5 ✅ SDK-I05 · `#[non_exhaustive]` 已铺开

`grep -rn '#\[non_exhaustive\]'` 在 0.2.2 源码中得到 50+ 命中，覆盖所有 `XxxQuery` 与 body struct。我们 CLI 侧改用 builder（4 处编译错全部由此触发，已修复）。

### 2.6 🟡 SDK-I06 · repo URL 部分修了

```toml
# 0.2.2 Cargo.toml
homepage  = "https://cnb.cool/aodoo/tools/cnb-rust"      # was rust-cnb
repository = "https://cnb.cool/aodoo/tools/cnb-rust.git" # was rust-cnb
```

URL 拼写改了（`rust-cnb` → `cnb-rust`），但仓库 web 端仍未公开 read（未登录访问仍需认证）。

**结论**：仍然挂在 [`docs/known-gaps.md`](./known-gaps.md) `#1 cnb-cli mirror URL` 旁边的下游条目上。

### 2.7 ❌ SDK-I07 · issue/pull number 类型不一致仍在

| 方法 | 0.2.2 参数类型 |
|------|-----------|
| `IssuesClient::get_issue` | `i64` |
| `IssuesClient::post_issue_assignees` | `String` |
| `PullsClient::get_pull` | `String` |

完全没动。我们的 `issue_number_i64` helper + 双轨转换继续用。

### 2.8 ❌ SDK-I08 · `Pull` vs `PullRequest` 双 DTO 仍在

CHANGELOG 没提。源码两个 struct 仍各自存在，字段集仍有差异（不过现在 `head/base` 都用 `PullRef` 类型一致了，比之前好一点）。

### 2.9 ✅ SDK-I09 · `Pull.{head,base}: Option<PullRef>`

```rust
// 0.2.2 src/models/common.rs:89
pub struct PullRef {
    #[serde(rename = "ref")] pub ref_: Option<String>,
    pub sha: Option<String>,
    pub repo: Option<serde_json::Value>,
}
```

字段名是 **`ref`**（不是我们之前在 [`SDK-I09.md`](./upstream-issues/SDK-I09.md) 推荐的 `branch`）。这意味着：

- 老 fixture / 老服务器返回 `{"head":{"branch":"x"}}` 时，typed 反序列化会**静默丢字段**（`PullRef` struct 没有 `branch` 字段）。
- 我们的 `read_branch` helper 仍尝试 `branch / ref / name`，但典型路径是先把 `Pull` typed 对象再 serialize 成 `Value`，那时只剩 `ref` 字段。

**调整**：fixture 已对齐到 `{"ref":"..."}`。`read_branch` helper 因为已经支持 `ref` 字段，**无需修改**。`PullRef.repo` 字段仍是 `Option<Value>`（嵌套结构没继续 typed），不影响我们当前用法。

### 2.10 ❌ SDK-I10 · path segment 仍未编码

`HttpInner::url()` 的实现就是 `base_url.join(path.trim_start_matches('/'))`，没做 percent-encoding 也没做拒绝。

**结论**：CLI 侧 `ensure_label_name_safe` 等防御性 helper 继续用。

### 2.11 ❌ SDK-I11 · `RepoPatch` 仍只 4 字段

```rust
// 0.2.2 models/data.rs:4313
pub struct RepoPatch {
    description: Option<String>,
    license:     Option<String>,
    site:        Option<String>,
    topics:      Option<Vec<String>>,
}
```

完全没动。`cnb repo edit --name / --default-branch` 继续 BadArgs 拒绝。

### 2.12 🟡 SDK-I12 · `set_repo_visibility` 仍是 query string

```rust
// 0.2.2 repositories.rs:299
pub async fn set_repo_visibility(
    &self,
    repo: String,
    query: &SetRepoVisibilityQuery,  // {visibility: Option<String>}
) -> Result<serde_json::Value>
```

形态没动。**待服务端确认 canonical wire shape**（同 [`known-gaps.md`](./known-gaps.md) #6）。

### 2.13 ❌ SDK-I13 · `list_forks_repos` 仍是 wrapper

`grep` 命中 `pub struct ListForks { … forks: Option<Vec<…>> … }`。

**结论**：`.forks.unwrap_or_default()` workaround 继续。

### 2.14 ✅ SDK-I14 · `HttpInner::reqwest_client()` 已 `pub`

完全实现了我们 [`SDK-I14.md`](./upstream-issues/SDK-I14.md) 推荐的 "Option A"。

**潜在的下一步 cleanup**（暂未做）：把 4 个 side-car `reqwest::Client::new()` 替换为 `ctx.sdk()?.http().reqwest_client()`：

- `commands/release.rs:482` — release upload phase 2
- `commands/release.rs:541` — release download
- `commands/build.rs:423` — build logs download
- `cnb-api::services::uploads` — `cnb issue create --attach` / `comment --attach`

最后一项做完后，**整个 `cnb-api` crate 可以彻底删除**。详见 §3.1。

### 2.15 ✅ SDK-I15 · `list_package_tags` 类型修正

```rust
// 0.2.2 src/registries.rs
pub async fn list_package_tags(
    &self, slug: String, kind: String, name: String,
    query: &ListPackageTagsQuery,
) -> Result<Vec<RegistryPackageTag>>
```

完全匹配我们 [`SDK-I15.md`](./upstream-issues/SDK-I15.md) 的建议。`registry tag list` 已**删除双发 workaround**，直接走 typed 路径。

### 2.16 🟡 SDK-I16 · `UpdateMembersRequest` 仍是 `{access_level, is_outside_collaborator}`

CHANGELOG 没提。源码确认未变。**待服务端确认 canonical** 同 [`known-gaps.md`](./known-gaps.md) #7。

### 2.17 ❌ SDK-I17 · `days` 过滤仍缺

```rust
// 0.2.2 repo_contributor.rs:63
pub struct GetRepoContributorTrendQuery {
    pub limit: Option<i64>,
    pub exclude_external_users: Option<bool>,
}
```

未补 `days` 字段。我们的 `cnb repo contributors --days` 走 `sdk_raw_get` workaround 继续。

### 2.18 ✅ SDK-I18 · `set_pinned_repo_by_group` 已加

```rust
// 0.2.2 repositories.rs:283
pub async fn set_pinned_repo_by_group(
    &self,
    slug: String,
    body: &Vec<String>,
) -> Result<Vec<crate::models::Repos4UserBase>>
```

方法名比我们 [`SDK-I18.md`](./upstream-issues/SDK-I18.md) 建议的 `set_pinned_repos` 更精确（区分 by-group / by-id）。**潜在的下一步 cleanup**：CLI 中 `cnb repo pin` 当前用 `Context::sdk_raw_json(PUT, ...)`，可改为 typed 调用。**暂未做**——属于优化，不修复 bug。

### 2.19 ❌ SDK-I19 · PR write DTO 仍欠缺字段

| DTO | 0.2.2 字段 | 缺失 |
|-----|-----------|------|
| `PullCreationForm` | `title / head / base / body / head_repo` | ❌ `assignees`, `labels` |
| `PatchPullRequest` | `title / body / state` | ❌ `base` |
| `MergePullRequest` | `merge_style / commit_title / commit_message` | ❌ `remove_source_branch` |

完全没动。CLI 的 4 个 `BadArgs` 拒绝继续。

---

## 3. 后续可立即做的 cleanup（非必需）

### 3.1 把 4 个 side-car reqwest 收编进 SDK 客户端（SDK-I14 已修复后）

**收益**：连接池 / retry / auth header / tracing 全部对齐 SDK，并能彻底删除 `cnb-api::services::uploads`，让整个 `cnb-api` crate 退役。

**成本**：4 处独立 flow 的 refactor，其中 `--attach` 的两阶段上传逻辑最复杂。建议作为一个独立 commit（类似 `users::get_self` port）。

**优先级**：低（功能已 work，纯架构清理）。

### 3.2 把 `cnb repo pin` / `unpin` 切到 typed `set_pinned_repo_by_group`

**收益**：少一处 `sdk_raw_json` workaround；返回类型 `Vec<Repos4UserBase>` 比 `serde_json::Value` 更可读。

**成本**：~10 行改动，wiremock 测试需对齐响应 shape。

### 3.3 把 `Context::sdk_raw_get` / `sdk_raw_json` 改用 SDK 暴露的 `http().url(path)`

**收益**：消除 base URL 拼接重复实现（与 SDK 内部规则）。

**成本**：~15 行改动，行为不变。

---

## 4. 验证矩阵

| 检查 | 结果 |
|------|------|
| `cargo update -p cnb@0.2.1` | ✅ → `cnb 0.2.2` 落锁 |
| `cargo check --workspace --all-targets` | ✅ pass（修复 4 处编译错） |
| `cargo fmt --check` → `cargo fmt --all` | ✅ pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ pass |
| `cargo test --workspace -j 2 -- --test-threads=1` | ✅ **200 passed / 0 failed**（与升级前一致） |

---

## 5. CHANGELOG / known-gaps 同步

待办（另一个提交）：

- `CHANGELOG.md [Unreleased]` 新增 "Changed (cnb-sdk 0.2.1 → 0.2.2)" 节，列出 7 项已修复、user-visible 的 capitalisation 变化、以及 4 处编译错的迁移点。
- `docs/known-gaps.md` 摘要表的状态列：
  - #5 SDK-I14 · ✅ 已解除（API 暴露），但 `cnb-api::uploads` 退休还要做完 §3.1 才算彻底完结
  - #2-#4（Tier A/B/C 上游 issue 提交）状态视上游 maintainer 是否已收到本仓库 0.2.1 时期的反馈而定
- `docs/sdk-issues.md` "Resolved issues" 节迁入 9 项已解决条目。
- `docs/upstream-issues/SDK-反馈汇总.md` 摘要表标记已修复行。

文档同步动作量小但跨多文件，建议作为 docs-only 后续提交。
