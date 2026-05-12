# Contributing to cnb-cli · 贡献指南

> Thanks for taking the time to contribute! 感谢你愿意贡献。
>
> 本文档是**入口级**贡献指南。深度的工程细节（架构、模块、加新命令的完整教程）在 [`docs/wiki/`](docs/wiki/) 项目知识库里，我们会在合适的小节里指过去，不在这里重复。

---

## 目录

1. [行为准则](#1-行为准则)
2. [我可以贡献什么](#2-我可以贡献什么)
3. [报 bug · 提 feature](#3-报-bug--提-feature)
4. [提 Pull Request 的标准流程](#4-提-pull-request-的标准流程)
5. [本地开发环境](#5-本地开发环境)
6. [代码风格与约定](#6-代码风格与约定)
7. [Commit message 约定（Conventional Commits）](#7-commit-message-约定conventional-commits)
8. [测试要求](#8-测试要求)
9. [文档贡献](#9-文档贡献)
10. [License & 法律](#10-license--法律)
11. [常见问题（FAQ）](#11-常见问题faq)

---

## 1. 行为准则

我们采用宽松友好的协作风格，核心两条：

- **对事不对人**：评审 PR、讨论方案时，针对**代码 / 设计 / 数据**，不针对个人
- **公开优先**：技术决策走 PR / issue / wiki，避免"私聊达成的隐性共识"导致后来人无法溯源

如果你遇到不当行为或感觉不被尊重，可以直接联系项目作者（见 [`Cargo.toml`](Cargo.toml) `authors`）私下处理。

---

## 2. 我可以贡献什么

按门槛从低到高：

| 类型 | 举例 | 难度 |
|------|------|:----:|
| **报 bug / 提 feature** | issue 一条 | ⭐ |
| **修文档 typo / 补 example** | docs/ / README / wiki / 源码注释 | ⭐ |
| **加测试用例** | wiremock 集成测试覆盖现有命令的 corner case | ⭐⭐ |
| **修小 bug** | `Repos4UserBase.flags` 一类的 SDK 漂移、TTY 渲染问题 | ⭐⭐ |
| **加新子命令** | 给某个命令组加一个新动词 | ⭐⭐⭐ |
| **加新命令组** | 全新资源类型（罕见，需先讨论） | ⭐⭐⭐⭐ |
| **架构演进** | 例如重做 HTTP 层、加 cache、TUI 模式 | ⭐⭐⭐⭐⭐ |

**强烈建议**：⭐⭐⭐ 以上的改动**先开 issue 讨论**再写代码，避免做完发现方向不被接受。

---

## 3. 报 bug · 提 feature

### 3.1 报 bug

请提供以下信息（缺哪一条都会先 bounce）：

```text
- cnb --version 输出
- 你的 OS（macOS arm64 / Linux x86_64 / ...）
- 完整可复现命令（含 -vv 输出，stderr 也要）
- 期望行为 vs 实际行为
- 如果是网络相关问题：cnb api <对应 path> 的原始返回（用于判断是 SDK 解码问题还是服务端形态问题）
```

**先自查**：

- 翻 [`docs/known-gaps.md`](docs/known-gaps.md) —— 你遇到的可能已是登记在册的"外部依赖型 open item"
- 翻 [`docs/sdk-issues.md`](docs/sdk-issues.md) —— 19 项已知 SDK 痛点是否覆盖
- 用 `cnb api <path>` 直接调一下 cnb 平台 API，看是不是服务端真的没数据 / 返回奇怪 shape（参考 commit `c93f01d` 的修复路径）

### 3.2 提 feature

| 形态 | 提的姿势 |
|------|--------|
| **小改进**（如 `cnb repo list` 加列） | 直接写在 issue body，3-5 行说清动机 |
| **新子命令** | issue body 列：用例场景、对应 cnb 平台 API 端点、与现有命令的语义边界 |
| **新命令组**（罕见） | 需要先评估该资源是否在 v1.0 路线图内（DESIGN §15）—— 不在路线图的话考虑做成 alias / `cnb api` 包装而非内置命令 |
| **架构改进** | 强烈建议先在 issue 写一份 RFC 风格的简短说明（动机 / 替代方案 / 取舍） |

---

## 4. 提 Pull Request 的标准流程

```mermaid
flowchart LR
    A[fork & branch] --> B[本地 cargo build/test]
    B --> C[fmt + clippy + test 三件套]
    C --> D[commit message<br/>Conventional Commits]
    D --> E[更新 CHANGELOG/wiki<br/>(如适用)]
    E --> F[push & open PR]
    F --> G[CI 通过]
    G --> H[评审 + 反馈循环]
    H --> I[merge]
```

### 4.1 PR 描述模板（强烈建议）

```markdown
## 改动动机
（这个 PR 解决什么问题？关联 issue #N？）

## 改动概要
- 改了什么
- 没改什么（避免 reviewer 误解 scope）

## 验证
- [ ] cargo fmt --check
- [ ] cargo clippy --workspace --all-targets -- -D warnings
- [ ] cargo test --workspace
- [ ] （如适用）mdbook build docs/
- [ ] （如适用）手动 smoke：`cnb <my-cmd> ...`

## 是否破坏向后兼容
- [ ] 否
- [ ] 是 → 在 commit message 里加 `!` 并说明 BREAKING CHANGE
```

### 4.2 PR 范围（scope）

| 推荐 | 不推荐 |
|------|------|
| 1 个 PR 解决 1 件事 | 一个 PR 顺便重构 + 修 bug + 改文档 |
| <500 行 diff | >2000 行 diff（除非是 generated 代码 / 大量测试 fixture） |
| commit 历史可读（不用改强制 squash） | 一堆 `wip` / `fix typo` 散乱 commit |

如果改动确实大，**拆成多个 PR**，第一个 PR 描述里说明后续会跟哪几个。

### 4.3 PR 评审节奏

- **CI 红了不要 ping reviewer** —— 自己先修绿
- **每轮反馈 1 个工作日内回复** —— 不行就在 PR 说明大致何时能跟进，避免 reviewer 误以为弃坑
- **不要 force-push 已 review 的内容** —— 加新 commit 让 reviewer 能看 diff；最后合入时由 maintainer 决定 squash 与否

---

## 5. 本地开发环境

### 5.1 一键就绪

```bash
git clone https://cnb.cool/cnb/cli   # 或你的镜像
cd cli
rustup show                           # 应显示 1.86+（rust-toolchain.toml 锁定）
cargo build --workspace               # 首编约 2 分钟
cargo test --workspace -j 2 -- --test-threads=1   # 全套 179 测试，约 1 分钟
```

### 5.2 装到 PATH（便于反复手测）

```bash
cargo install --path crates/cnb --locked --force
cnb --version                         # → cnb 0.4.0-alpha.1
```

每次代码改动**重跑** `cargo install --path crates/cnb --locked --force` 就能让 `~/.cargo/bin/cnb` 用上最新版本。

### 5.3 用 wiremock 测试某个命令

参考 `crates/cnb/tests/m2_*.rs` / `m3_*.rs` / `m4_*.rs` 的现有用例。骨架在 [`docs/wiki/06-developer-guide.md`](docs/wiki/06-developer-guide.md) §6.5 "测试约定" 段。

### 5.4 跑 mdbook docs 站本地预览

```bash
cargo install mdbook --locked --version "^0.4"   # 一次性安装
cd docs && mdbook serve --open                    # 自动开浏览器
```

---

## 6. 代码风格与约定

### 6.1 强制规则（CI 会拒）

| 工具 | 命令 | 说明 |
|------|------|------|
| **rustfmt** | `cargo fmt --check` | 用 workspace `rustfmt.toml` 配置 |
| **clippy** | `cargo clippy --workspace --all-targets -- -D warnings` | warning 也是错误 |
| **cargo deny** | `cargo deny check`（如安装） | license / advisory / 重复依赖检查 |

### 6.2 软约定（review 会指出）

- **`unsafe` 默认禁用**（workspace `clippy.toml` 已配置）
- **不直接 `reqwest::Client::new()`** —— 用 `ctx.sdk()?.http().reqwest_client()`（详见 [wiki 02](docs/wiki/02-architecture.md) § HTTP 路径）
  - **唯一例外**：`commands/release.rs` 的 release upload phase 2，pre-signed URL 不能带 Authorization header；源码已有显眼注释
- **不在 commands 里直接读 env** —— 通过 `Context` 中转
- **新 public API 必须有 `///` 文档注释 + doc-test**
- **TTY vs pipe 行为分支** —— 表格 / JSON / progress bar 都按 `ctx.io.stdout_is_tty` 判断（详见 [wiki 03 § cnb-tty](docs/wiki/03-modules.md#37-cratescnb-tty420-行)）
- **错误归一到 `CliError`** —— 不要在命令里 `panic!` / `unwrap` 用户可能触发的路径

### 6.3 命名

- 模块文件：`snake_case.rs`
- 命令子动词：`kebab-case`（与 `gh` 对齐：`set-visibility` / `delete-logs`）
- 环境变量：`SCREAMING_SNAKE_CASE` + `CNB_` 前缀
- 退出码：见 [`crates/cnb-cli/src/error.rs::exit_code`](crates/cnb-cli/src/error.rs) + DESIGN §12

---

## 7. Commit message 约定（Conventional Commits）

格式：

```text
<type>(<scope>)[!]: <subject 50 字符内>

<可选 body，70 字符 wrap>

<可选 BREAKING CHANGE: 段、Closes #N 段>
```

### 7.1 type 一览（按本仓库实际使用频率排）

| type | 用途 | 例子 |
|------|------|------|
| `feat` | 新功能 | `feat(issue): add --attach to comment` |
| `fix` | bug 修复 | `fix(repo,search): drop typed Repos4UserBase decoding` |
| `docs` | 文档 only | `docs(wiki): generate CodeWiki-style knowledge base` |
| `refactor` | 不改外部行为的重构 | `refactor(cnb-cli): retire the cnb-api crate` |
| `deps` | 依赖升降 | `deps(cnb-sdk): upgrade to cnb 0.2.2` |
| `test` | 加 / 改测试 | `test(repo): pin wire shape for set-visibility` |
| `chore` | 杂项（CI、build 配置） | `chore(ci): bump rust-toolchain to 1.86` |
| `perf` | 性能优化 | `perf(release): parallelise upload phase 2 (concurrency=4)` |

### 7.2 scope（圆括号里）

通常是 crate 名 / 命令组 / 文档名：

- `cnb-cli` / `cnb-auth` / `cnb-config` / `cnb-git` / `cnb-tty`
- `repo` / `issue` / `pr` / `build` / `release` / `auth` / `api` / ...
- `wiki` / `readme` / `design` / `known-gaps` / `sdk-0.2.2-upgrade`
- 多 scope 用逗号：`feat(issue,pr)` / `fix(repo,search)`

### 7.3 BREAKING CHANGE

在 type 后加 `!`，**且** body 里有 `BREAKING CHANGE:` 段：

```text
feat(issue)!: invert 'list' default scope; drop --mine flag

BREAKING CHANGE (CLI): `cnb issue list` (no slug, no flag) now lists
issues ACROSS ALL REPOS accessible to the current token — backed by
GET /user/issues. To list issues in one repo, pass the slug
explicitly: `cnb issue list OWNER/REPO`.
```

### 7.4 body 写什么

- **why**（为什么改）—— 这是最有价值的部分，1 年后回看 `git log` 救命
- **how**（怎么改的简要描述，diff 太长时尤其重要）
- **取舍**（为什么不选 plan B）
- **关联**（`Closes #N` / `Refs #M` / `Tracked as docs/known-gaps.md #16`）

参考样本：`git log --format=%B 2e4200f` 是一个完整结构样本。

---

## 8. 测试要求

| 改动类型 | 必须的测试 |
|--------|----------|
| 新 typed call | wiremock 集成测试（`crates/cnb/tests/`） |
| 新 raw 调用 | 同上 + 在源码注释里说明走 raw 的原因 |
| 修 bug | **先写一个能复现 bug 的失败测试**，再改源码让它通过 |
| 新 helper / 纯函数 | 单元测试（`#[cfg(test)] mod tests`） |
| 新 public API | doc-test（`///` 块里的 `# Examples`） |
| 修文档 | 不需要测试，但 mdbook build 必须仍干净 |

约定细节见 [wiki 06 § 6.5](docs/wiki/06-developer-guide.md#65-测试约定)。

---

## 9. 文档贡献

cnb-cli 的文档分四套，受众不同：

| 文档 | 受众 | 改的时候注意 |
|------|------|------|
| [`README.md`](README.md) / [`README.zh-CN.md`](README.zh-CN.md) | 第一次访问的人 | 中英文要同步改 |
| [`docs/src/`](docs/src/)（mdbook） | 终端用户 | 修后跑 `mdbook build` 验证 |
| [`docs/wiki/`](docs/wiki/) | 接手项目的工程师 | 跨文档链接要保持有效；锚定 commit 写明 |
| [`DESIGN.md`](DESIGN.md) | 想了解 M0 设计意图的人 | **史料档案**，不更新当前态（详见 DESIGN §0） |

**不要为了"对齐"在多处复制同一段** —— 选一个 SoT，其它地方链过去。否则会漂移。

### 9.1 加新文档

| 类型 | 放哪 |
|------|------|
| 用户手册条目 | `docs/src/`，在 `docs/src/SUMMARY.md` 加目录条目 |
| 工程视角文档 | `docs/wiki/`，在 `docs/wiki/README.md` 加 TOC 条目 |
| SDK 上游 issue 草稿 | `docs/upstream-issues/`，按现有 19 份的格式 |
| 升级核对报告 | `docs/sdk-X.Y.Z-upgrade.md`，按 `docs/sdk-0.2.2-upgrade.md` 模板 |

---

## 10. License & 法律

cnb-cli 采用 **MIT OR Apache-2.0** 双授权（你可以任选一个适用）。详见仓库根的 [`LICENSE-MIT`](LICENSE-MIT) 和 [`LICENSE-APACHE`](LICENSE-APACHE)。

提交 PR 即视为你同意以同样的 license 释出你的贡献（**inbound = outbound**，与多数 Rust 社区项目相同），无需另签 CLA。

如果你的改动**复制 / 衍生**了其它 license 不兼容的代码（GPL / AGPL / 自定义专有 license），请**不要**直接提 PR，先在 issue 里讨论。

---

## 11. 常见问题（FAQ）

### Q1: 我改了源码，但 `cnb` 命令还是旧行为？

A: 重装：`cargo install --path crates/cnb --locked --force`。`cargo install` 不能感知业务代码改动，必须 `--force`。

### Q2: CI 红在 clippy `-D warnings`，但我看不出哪里 warning？

A: 本地跑 `cargo clippy --workspace --all-targets -- -D warnings`（注意 `--all-targets`，少了它会漏 test 的 lint）。

### Q3: wiremock 测试不稳定 / 偶发挂掉？

A: 必须 `--test-threads=1`（多个测试同时起 mock server 会冲突）；并且每个测试起独立 `MockServer::start().await`，**不要**复用全局。

### Q4: 我想加一个 SDK 还没暴露的 endpoint？

A: 走 `Context::sdk_raw_get` / `sdk_raw_json` / `sdk_raw_get_bytes`，参考 `commands/repo.rs::list` 的 `flags` workaround 模板（必带源码注释说明原因）。详细套路见 [wiki 06 § 6.4](docs/wiki/06-developer-guide.md#64-sdk-schema-漂移兜底套路)。

### Q5: 项目装好但 `cnb auth login` 提示 keyring 失败？

A: 设 `CNB_KEYRING_BACKEND=memory` 临时绕过（仅测试用，不持久），或在 macOS / Linux 下确认 system keychain 服务可用。Linux 需要 `gnome-keyring` / `kwallet` 之一在跑。

### Q6: 我想跑跨平台 build（aarch64 / x86_64 / Linux / Windows）？

A: 项目用 `release.yml` 流水线统一出包；本地不需要交叉编译。如果你确实要本地试 Windows binary，用 `cross` crate 或 `cargo zigbuild`。

---

## 推荐阅读顺序（如果你是首次贡献）

1. 本文 § 4–§ 7 —— 知道 PR 怎么提
2. [`docs/wiki/01-project-overview.md`](docs/wiki/01-project-overview.md) —— 5 分钟知道项目是什么
3. [`docs/wiki/06-developer-guide.md`](docs/wiki/06-developer-guide.md) —— 30 分钟加你的第一个命令
4. （遇到具体疑问时）[`docs/wiki/02-architecture.md`](docs/wiki/02-architecture.md) / [`docs/wiki/03-modules.md`](docs/wiki/03-modules.md) / [`docs/wiki/04-command-catalog.md`](docs/wiki/04-command-catalog.md) / [`docs/wiki/05-data-flows.md`](docs/wiki/05-data-flows.md)

**Welcome aboard. 期待你的 PR。**
