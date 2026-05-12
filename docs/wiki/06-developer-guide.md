# 06 · 二次开发指南

> 写给"准备给 cnb-cli 加新命令、改架构、提 PR"的人。
> **读完你能做到**：30 分钟加一个新子命令、跑通本地全套验证、提一个干净的 PR。

---

## 6.1 30 秒环境就绪

```bash
git clone https://cnb.cool/cnb/cli   # 或你的镜像
cd cli
rustup show                           # 确认 1.86+（rust-toolchain.toml 锁定）
cargo build --workspace               # 第一次编译约 2 分钟
cargo test --workspace                # 全套 179 测试约 1 分钟

# 装到 ~/.cargo/bin（开发期可重复跑）
cargo install --path crates/cnb --locked --force
cnb --version    # → cnb 0.4.0-alpha.1
```

**强制约定**：

- 不动 `Cargo.lock`（除非升级依赖）
- 不调 `unsafe`（workspace clippy 配置 `-D unsafe_code` 会拒）
- 不直接 `reqwest::Client::new()`（除了 release upload phase 2 例外，源码注释明示）

---

## 6.2 加一个新子命令的标准流程

假设要加 `cnb repo stars <slug>` —— 显示某仓库的 star 数。

### Step 1：确认 SDK 是否有 typed 接口

```bash
# 在本地 cargo registry 里搜
grep -r "fn list_stars\|fn get_stars\|stars" \
  ~/.cargo/registry/src/index.crates.io-*/cnb-0.2.2/src/repositories.rs
```

- **有** → typed call，跳到 Step 2
- **没有但服务端有** → `Context::sdk_raw_get` 兜底，跳到 Step 2
- **服务端也没有** → 提一个 known-gaps 条目（见 6.6）

### Step 2：在 `commands/repo.rs` 加 enum 分支

```rust
// commands/repo.rs

#[derive(Debug, Subcommand)]
pub enum RepoSub {
    // ... existing ...
    /// Show star count for a repo.
    Stars(StarsArgs),
}

#[derive(Debug, Args)]
pub struct StarsArgs {
    /// Repo slug (`OWNER/REPO[/SUBPATH]`). Defaults to current git remote.
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

pub async fn run(ctx: &mut Context, args: RepoArgs) -> Result<(), CliError> {
    match args.cmd {
        // ... existing ...
        RepoSub::Stars(a) => stars(ctx, a).await,
    }
}

async fn stars(ctx: &mut Context, args: StarsArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // typed 调用示例：
    let client = ctx.sdk()?;
    let dto = client.repositories().get_repo_stars(repo).await?;
    let v = serde_json::to_value(&dto).expect("stars DTO serialises infallibly");

    if render(ctx, &args.out, &v)? {
        return Ok(());  // --json/--jq/--template 已处理
    }

    // 默认 TTY 渲染
    let count = v.get("count").and_then(Value::as_i64).unwrap_or(0);
    println!("{count}");
    Ok(())
}
```

### Step 3：写 wiremock 集成测试

`crates/cnb/tests/m2_repo.rs`（或新建 `m4_repo_stars.rs`）：

```rust
#[tokio::test]
async fn repo_stars_renders_count() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/stars"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1234
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "stars", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.trim(), "1234");
}
```

### Step 4：本地全套验证

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -j 2 -- --test-threads=1
# 三个都 exit 0 才能提 PR
```

### Step 5：更新 wiki / changelog

| 文件 | 改什么 |
|------|------|
| [`docs/wiki/04-command-catalog.md`](./04-command-catalog.md) | `repo` 段加一行 |
| [`CHANGELOG.md`](../../CHANGELOG.md) `[Unreleased]` | 加 `Added: cnb repo stars ...` |
| [`README.md`](../../README.md) / [`README.zh-CN.md`](../../README.zh-CN.md) | 大特性才加；小子命令免 |

---

## 6.3 加一个新 crate 的高门槛检查

**默认答案：不要加新 crate**。当前 6 个 crate 已经覆盖所有边界（auth / config / git / tty / cli / bin）。新加一个 crate 必须满足以下**至少 2 条**：

1. **能在 cnb-cli 之外被复用**（例如 `cnb-graph` 给 GraphQL 客户端，但平台还没 GraphQL）
2. **测试隔离强需求**（mock 困难 / 跑得慢 / 拉额外大依赖）
3. **明确的边界守卫**（这块代码不在新 crate 里就会侵蚀其它模块）

如果只是"代码太多想拆"，正确做法是**继续在 cnb-cli 里分 module**（例如 `commands/repo.rs` 已经 758 行，如果到 1500 行可以拆 `commands/repo/{list,view,create,...}.rs`）。

---

## 6.4 SDK schema 漂移兜底套路

发现 `cargo run -- some-cmd` 报 `invalid type: ..., expected struct ...` 时：

1. 在 `cnb api` 里直接 raw 跑同 path，看服务端实际返回 shape
2. 对照 SDK DTO（`cargo doc --open --package cnb-sdk` 或直接读 `~/.cargo/registry/src/.../cnb-0.2.2/src/models/data.rs`）
3. **不改 SDK**（它是上游 crate），用 `Context::sdk_raw_*` 绕开
4. 在源码里写显眼注释（参考 `commands/repo.rs::list` 的注释段，或 `commands/search.rs::run` 的同款）：
   - 标注 cnb-sdk 版本
   - 标注 wire 实际形态 vs DTO 期望
   - 标注哪些字段被丢弃（确认无碍后）
   - 标注未来 SDK 修复后如何回退到 typed call（留 marker）
5. **更新 `docs/known-gaps.md`** 摘要表 + §2 详情段（参考 #16 的格式）
6. **加 wiremock 测试 pin 当前 wire 形态** —— 未来 SDK 修复后该测试会自动验证行为一致

---

## 6.5 测试约定

### 单元测试（`#[cfg(test)] mod tests`）

| 类型 | 例子 |
|------|------|
| 纯函数 | `cnb-tty::jq::apply` / `cnb-config::hosts::default_user` |
| 格式化 | `format_visibility` / `format_issue_number` |
| trait mock | `InMemoryKeyring` 替换 `RealKeyring` |

### 集成测试（`crates/cnb/tests/*.rs`）

模板：

```rust
mod common;  // common::TestEnv, sets HOME=tempdir, CNB_TOKEN=fake

use wiremock::matchers::{method, path, body_partial_json};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde_json::json;

#[tokio::test]
async fn my_command_does_x() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/some/path"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({...})))
        .mount(&server).await;

    let env = common::TestEnv::new();
    let assert = env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())  // 把 SDK 重定向到 mock
        .args(["my-cmd", "..."])
        .assert()
        .success();

    // 断言 stdout / stderr / exit code
}
```

**强约定**：

- `--test-threads=1` 跑（wiremock 在并发下可能冲突）
- 每个测试起独立 `MockServer`（避免互相污染）
- `env.cmd()` 总是从 `common::TestEnv::new()` 拿，自动设 `HOME=tempdir` 隔离配置文件
- 不依赖网络（绝不出现 `cnb.cool` 真实主机）

### Doc-tests

cnb-auth / cnb-config / cnb-git / cnb-tty 的 lib `pub fn` 都附带 `///` 注释和 doc-test。`cargo test --doc` 自动跑。

---

## 6.6 项目治理流程

### 提一个 known-gap 条目

如果你的改动揭示了"本仓库内做不了，要等外部"的事项：

1. 编辑 [`docs/known-gaps.md`](../known-gaps.md)
2. 摘要表加一行（按现有 16 项的格式：条目 / 类别 / 阻塞原因 / 影响范围 / 解除条件）
3. 选合适的 §（§1 上游反馈 / §2 SDK 修复 / §3 基础设施 / §4 spec / §5 取舍），在 § 末尾追加详情段
4. 详情段必含 5 节：**现状** / **为什么阻塞** / **影响范围** / **解除条件** / **建议负责人**
5. 如果改动了源码，在源码里留个指向 known-gaps 编号的注释（参考 `commands/pr.rs::list` 的 `#16` 注释）

### 提一个 SDK upstream issue

详见 [`docs/sdk-issues.md`](../sdk-issues.md) 和 [`docs/upstream-issues/`](../upstream-issues/) —— 已有 19 项的现成模板。

### 给 cnb-sdk 升级（例如 0.2.3）

参考 [`docs/sdk-0.2.2-upgrade.md`](../sdk-0.2.2-upgrade.md) 的格式，写一份 `sdk-0.2.3-upgrade.md`：

1. § "结论先行" —— 几项已解决、几项仍 open
2. § "兼容性变化" —— Cargo.toml 改了什么、breaking 在哪
3. § "已修的 SDK 痛点" —— 逐项核对 docs/sdk-issues.md
4. § "follow-up cleanup" —— 哪些 raw 路径可以回退到 typed
5. § "验证矩阵" —— fmt + clippy + test + mdbook 全跑过
6. 同步：`docs/known-gaps.md` 状态切换 / `docs/sdk-issues.md` resolved 段 / `CHANGELOG.md`

---

## 6.7 CI/CD 约定

| 工具 | 何时跑 | 失败处理 |
|------|------|--------|
| `cargo fmt --check` | PR | 不通过不能 merge；`cargo fmt --all` 修 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PR | 不通过不能 merge；按 lint 提示修 |
| `cargo test --workspace -- --test-threads=1` | PR | 不通过不能 merge |
| `cargo deny check` | PR | license / advisories / 重复依赖红线 |
| `mdbook build docs/` | PR (docs only) | 文档站不能挂 |
| Release pipeline (`release.yml`) | tag `v*` 推送 | 多平台 build + cosign 签名 + Homebrew/Scoop manifest 渲染 |

---

## 6.8 常见陷阱（FAQ）

### Q1: 为什么我加的命令在 `cnb --help` 看不到？

A: 你忘了在 `crates/cnb-cli/src/cli.rs` 的 `Commands` enum 加 variant，或忘了在 `crates/cnb-cli/src/lib.rs::run` 的 match 加 arm。

### Q2: 集成测试里 `cnb` 死活跑真实 cnb.cool？

A: `CNB_API_BASE` env 没设到 wiremock URL，或 `Context::set_sdk_base_url` 没被 `TestEnv` 链入。看 `common.rs` 的实现，确保 env 链是 `CNB_API_BASE` → `Context::sdk_base_url` → `cnb_sdk::ClientBuilder::base_url`。

### Q3: 命令在 TTY 上花花绿绿，pipe 时全是颜色控制字符？

A: 渲染时漏判 `ctx.io.stdout_is_tty`。所有 `cnb-tty::table::write_table` / `json_out::write_json` 都接 `is_tty` 参数，按它分支即可。

### Q4: 我想加 retry，但只对某条命令？

A: typed SDK 已带 retry（exponential backoff + Retry-After honored）。如果你的命令走 `sdk_raw_*`，retry 也由 SDK `HttpInner` 自动加。**不要**在命令层手写 retry。如果走 `http::passthrough`，**故意不重试**（gh api 风格）。

### Q5: token 在 CI 里怎么传？

A: 设 `CNB_TOKEN` env 即可。**不要**在 CI 里跑 `cnb auth login`（dialoguer 会卡住）。

### Q6: 我改了 `commands/foo.rs` 但 `cargo install --path crates/cnb --force` 装的还是旧版？

A: cargo install 会比对 `cnb` bin 的 Cargo.toml 时间戳，业务代码改动它探测不到。加 `--force` 总是重装，或先 `cargo build --release --bin cnb` 一次。

---

## 6.9 联系入口

| 场景 | 谁 |
|------|---|
| 业务问题 / 设计讨论 | 项目作者（见 `Cargo.toml::authors`） |
| SDK 上游问题 | 跟 cnb-sdk crate 维护者（crates.io 上的 `cnb` 包）联系 |
| 平台 API 问题 | cnb 服务端团队（`docs/known-gaps.md` 多次提到的"服务端澄清"项） |
| 文档 typo | 直接发 PR，docs-only 加速 review |

---

**祝写码愉快**。建议路径：先读一遍这份 wiki 的 [01](./01-project-overview.md) → [02](./02-architecture.md) → [03](./03-modules.md)，然后按 6.2 的 5 步流程加你的第一个命令。
