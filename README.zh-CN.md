# cnb — CNB CLI

[![ci](https://cnb.cool/aodoo/tools/cnb-cli/-/badge/git/latest/ci/git-clone-yyds)](https://cnb.cool/aodoo/tools/cnb-cli)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可)

[CNB（CloudNative Build，cnb.cool）](https://cnb.cool)的官方命令行工具，使用 Rust 实现，命令风格与 GitHub `gh` CLI 对齐。

> **English version**: [README.md](./README.md)
>
> **状态**：v0.4.0-alpha — 17 个顶层命令；M4 已完成（registry / mission / org / browse / completion / config / alias）+ M5.0 升级检查与发布流水线。详见[里程碑状态](#里程碑状态)。

## 快速上手

```bash
# 1. 登录（按提示粘贴你的 CNB Personal Access Token）
cnb auth login

# 2. 验证
cnb auth status

# 3. 直接调用任意 OpenAPI 端点
cnb api /user
cnb api /user/repos --jq '.[].name'

# 4. M2：高层命令
cnb repo list                                  # 当前 token 可见的仓库
cnb repo view cnb/feedback                     # 仓库卡片
cnb issue list cnb/feedback --state=open
cnb issue create --title "bug" --body "..." cnb/feedback --attach screenshot.png
cnb issue close 42 cnb/feedback
cnb pr list cnb/feedback
cnb pr create --title "feat" --base main cnb/feedback   # head = 当前分支
cnb pr merge 7 cnb/feedback --method=squash --yes
cnb mr view 7 cnb/feedback                     # `mr` 是 `pr` 的别名
cnb label list cnb/feedback

# 5. M3：流水线、工作空间、发布
cnb build run cnb/feedback --branch main       # 触发一次构建
cnb build status sn-123 cnb/feedback --watch   # 轮询直到终态（ctrl-c 安全）
cnb build logs pipeline-456 cnb/feedback --output runner.log
cnb workspace list                              # 我的开发环境
cnb workspace start cnb/feedback --branch main # 在浏览器中打开 webide URL
cnb ws view --sn sn-x --web                    # `ws` 别名；--web 自动打开
cnb release list cnb/feedback
cnb release create v1.2.0 --repo cnb/feedback --notes "Bug fixes"
cnb release upload r-1 dist/app.tar.gz --repo cnb/feedback --clobber
cnb release download v1.2.0 app.tar.gz --repo cnb/feedback --output ./downloads
cnb pr review 7 cnb/feedback --approve --body "LGTM"
cnb issue properties 42 cnb/feedback --set sev=high --set area=backend

# 6. M4：制品仓库、任务、组织、浏览、补全、配置、别名
cnb registry list cnb                          # 组 `cnb` 下的制品仓库
cnb registry package list cnb --type npm
cnb registry tag view cnb --type npm --name foo --tag v1.0.0
cnb registry tag provenance cnb --type npm --name foo --tag v1.0.0
cnb mission view-list cnb/m1                   # 任务下的视图
cnb org list                                   # 我加入的组织
cnb org member add cnb alice --role write
cnb org follower alice                          # alice 的关注者
cnb browse                                      # 在浏览器中打开当前仓库
cnb browse --issue 42                           # 跳转到某个 issue
cnb browse --no-browser                         # 仅打印 URL
cnb completion zsh > ~/.zsh/completions/_cnb
cnb config set core.git_protocol ssh
cnb config list
cnb alias set bugs 'issue list -l bug'
cnb alias list
cnb alias import < team-aliases.toml
cnb auth setup-git                              # 配置 git credential helper

# 7. M5.0：保持 cnb 最新（仅 opt-in）
cnb update                                      # 检查 GitHub Releases 是否有新版本
cnb update --check                              # 静默，仅返回 yes/no
```

## 安装

### 一行命令安装（发版后推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh | bash

# 锁定指定版本 / 安装到自定义前缀：
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh \
  | bash -s -- --version v0.4.0-alpha.1 --prefix ~/.local/bin
```

安装脚本会自动检测平台、从 GitHub 下载匹配的发布归档、**校验 SHA-256 校验和**，然后安装二进制。也会读取 `CNB_VERSION` / `CNB_PREFIX` / `CNB_REPO` 环境变量。

### 从源码构建

```bash
git clone https://cnb.cool/aodoo/tools/cnb-cli
cd cli
cargo build --release
./target/release/cnb --help
```

需要 Rust 1.86+。

## 认证

Token 按以下顺序解析（详见 [DESIGN §5](DESIGN.md#5-认证子系统)）：

1. **`CNB_TOKEN`** 环境变量（CI / 容器场景）
2. **系统密钥环**（macOS Keychain / Windows Credential Manager / Linux Secret Service）
3. **`~/.config/cnb/hosts.toml`**（密钥环不可用时的兜底）

```bash
# CI 场景
export CNB_TOKEN=xxxxxxxxxxxxxxxxxxxxxxxx
cnb api /user

# 本地场景
cnb auth login          # 写入密钥环（或 hosts.toml）
cnb auth token          # 打印当前 token（供 pipe）
cnb auth logout         # 删除凭据
```

## `cnb api` —— 通用 REST 直通

与 `gh api` 对齐：

```bash
# GET
cnb api /user
cnb api /cnb/feedback/-/issues

# POST 带字段
cnb api -X POST /cnb/feedback/-/issues -f title='Bug' -f body='Details...'

# 自定义 header / 显示响应头 / 静默
cnb api /user -H 'X-Trace-Id: abc' -i
cnb api /user --silent

# 用 jq 或 template 后处理
cnb api /user --jq '.username'
cnb api /user --template '{username}: {email}'
```

## 文档

- [DESIGN.md](DESIGN.md) —— 架构、命令清单、路线图（设计意图的单一来源）。
- [docs/wiki/](docs/wiki/) —— **项目 Wiki**（CodeWiki 风格）：6 篇工程知识库（项目概述 / 系统架构 / 核心模块 / 命令清单 / 数据流 / 二次开发指南）。**接手项目首推入口**。
- [docs/cnb-pipeline.md](docs/cnb-pipeline.md) —— `.cnb.yml` 设计说明：cnb 平台流水线如何映射 / 偏离 `.github/workflows/` 中的 GitHub Actions 工作流。
- [docs/known-gaps.md](docs/known-gaps.md) —— **被外部依赖阻塞**的 open item（上游 SDK 修复、服务端澄清、托管基础设施）。给后续接手人 / 项目交接看的单页仪表盘。
- [docs/sdk-issues.md](docs/sdk-issues.md) —— 19 个上游 SDK 痛点 + rollout plan。中文整合稿见 [`docs/upstream-issues/SDK-反馈汇总.md`](docs/upstream-issues/SDK-反馈汇总.md)。
- [docs/sdk-0.2.2-upgrade.md](docs/sdk-0.2.2-upgrade.md) —— cnb-sdk 0.2.2 升级核对报告，含上游修复落地矩阵 + cnb-api crate 完全退役的过程。
- `cnb --help`、`cnb <command> --help` —— 内置帮助。

## 里程碑状态

| 里程碑 | 状态     | 范围                                                         |
| ------ | -------- | ------------------------------------------------------------ |
| **M0** | ✅ 完成  | 设计冻结（DESIGN.md v0.1）                                  |
| **M1** | ✅ 完成  | Workspace 骨架 + `cnb auth` + `cnb api`                      |
| **M2** | ✅ 完成  | `cnb repo` / `cnb issue` / `cnb label` / `cnb pr`（39 个子命令；progenitor 推迟到 M2.x） |
| **M3** | ✅ 完成  | `cnb build` / `cnb workspace` / `cnb release` + `cnb pr review/checks/batch` + `cnb issue activity/properties`（27 个新子命令） |
| **M4** | ✅ 完成  | `cnb registry` / `cnb mission` / `cnb org` + `cnb repo collaborator/pin/contributors` + `cnb browse` / `cnb completion` / `cnb config` / `cnb alias` + `cnb auth setup-git`（35+ 个新子命令） |
| **M5.0** | ✅ 完成 | `cnb update`、`release.yml`、`scripts/install.sh`、CI 加固 |
| **M5.1** | ✅ 完成 | man pages + 5 种 shell 的补全脚本打包进 release 归档；cosign keyless 签名；mdbook handbook 脚手架；Homebrew/Scoop manifest 模板 |
| **SDK-1** | ✅ 完成 | cnb-api → typed SDK 迁移的 Phase 1：依赖外部 crate `cnb = "0.2"`，通过新命令 `cnb search` 试点（首个消费者），其它命令保留原 `cnb-api` facade |
| **SDK-2** | ✅ 完成 | Phase 2 在 cnb 0.2.2 后续清理后**全部完成**：每一个 CLI 动词都通过 typed SDK；本地 `cnb-api` crate 已**完全退役**（workspace 7 → 6 个 crate）。SDK 不建模的两个 flow —— `cnb api` 原始直通 + `cnb issue --attach` multipart 上传 —— 现在落在 `cnb-cli::http` 模块下，构建于 `client.http().reqwest_client()` 之上。详见 [`docs/sdk-issues.md`](docs/sdk-issues.md) 的 19 个上游问题、[`docs/sdk-0.2.2-upgrade.md`](docs/sdk-0.2.2-upgrade.md) 的修复矩阵、[`docs/known-gaps.md`](docs/known-gaps.md) 中剩余的外部依赖项 |
| **M5.2** | partial | apt / yum repos、Docker image —— 外部基础设施（详见 [known-gaps #9](docs/known-gaps.md)） |
| **M6** | partial | sigstore 签名 ✅；mdbook 部署 + 外部 case study —— 外部依赖（详见 [known-gaps #10 / #11](docs/known-gaps.md)） |

### M1 验收报告

| # | 验收项                                                                  | 状态 |
|---|------------------------------------------------------------------------|------|
| 1 | `cargo build --workspace` 成功                                         | ✅   |
| 2 | `cargo test --workspace` 通过（72 个单元 + 集成测试）                  | ✅   |
| 3 | `cargo fmt --check` 与 `cargo clippy -D warnings` 干净                 | ✅   |
| 4 | `cnb --help` 暴露 `auth` 与 `api` 子命令                               | ✅   |
| 5 | `CNB_TOKEN=… cnb api /user` 在真实 `cnb.cool` 上返回用户 JSON          | ✅   |
| 6 | `cnb auth login --with-token` 写入密钥环，`auth status` 能读出         | ✅   |
| 7 | 密钥环不可用时回退到 `hosts.toml`，`0600` 权限                          | ✅   |
| 8 | `cargo xtask sync-openapi` 写出 `openapi/cnb-swagger-2.0.json`         | ✅   |
| 9 | 没有 `crates/cnb-api/src/generated/`（推迟到 M2 配合 progenitor）       | ✅   |
| 10| README quickstart 文档化了 login → api `/user` 的流程                   | ✅   |

### M2 验收报告

| # | 验收项                                                                                          | 状态 |
|---|-----------------------------------------------------------------------------------------------|------|
| 1 | `cargo build --workspace --all-targets` 成功                                                  | ✅   |
| 2 | `cargo test --workspace -- --test-threads=1` 通过（114 个测试）                                | ✅   |
| 3 | `cargo clippy --workspace --all-targets -- -D warnings` 干净                                  | ✅   |
| 4 | `cnb --help` 暴露 `repo`/`issue`/`label`/`pr`（+`mr` 别名）                                    | ✅   |
| 5 | `cnb repo` 11 个子命令；`cnb issue` 11 个；`cnb label` 4 个；`cnb pr` 13 个                    | ✅   |
| 6 | 所有 path 都走 `cnb-api::url_safe::resolve`（任何地方都不直接拼字符串构造 URL）                | ✅   |
| 7 | `Context::resolve_repo` 优先 `--repo OWNER/REPO`，回落 `git remote get-url origin`            | ✅   |
| 8 | 破坏性操作（`repo delete`、`pr merge`、`label delete`、`repo transfer`）需 `--yes`/TTY        | ✅   |
| 9 | 附件上传器自动识别 file vs image，使用 `tokio-util::ReaderStream` 流式上传                     | ✅   |
| 10| 仍未引入 `crates/cnb-api/src/generated/` —— progenitor 集成推迟到 M2.x                        | ⏭   |

### M3 验收报告

| #  | 验收项                                                                                          | 状态 |
|----|-----------------------------------------------------------------------------------------------|------|
| 1  | `cargo build --workspace --all-targets` 成功                                                  | ✅   |
| 2  | `cargo test --workspace -- --test-threads=1` 通过（151 个测试，0 失败）                        | ✅   |
| 3  | `cargo clippy --workspace --all-targets -- -D warnings` 干净                                  | ✅   |
| 4  | `cnb --help` 暴露 3 个新子命令：`build`/`workspace`（+`ws` 别名）/`release`                    | ✅   |
| 5  | `cnb build` 8 个子命令含 `--watch`（tokio interval + indicatif spinner + ctrl-c）              | ✅   |
| 6  | `cnb workspace` 5 个子命令；`start` 通过 `open` crate 自动在浏览器打开 webide URL              | ✅   |
| 7  | `cnb release` 9 个子命令含两阶段资源上传（URL → PUT → confirm）                                | ✅   |
| 8  | `cnb pr review/checks/batch` 与 `cnb issue activity/properties` 扩展 M2 命令                   | ✅   |
| 9  | 新 service facade（`builds`/`workspaces`/`releases`）13 个 wiremock 单测覆盖                   | ✅   |
| 10 | M3 集成测试覆盖 TSV 输出、两阶段上传、别名解析、`--watch` 管道                                  | ✅   |

**Crate 列表（M1 时）：** `cnb`（bin）· `cnb-cli` · `cnb-api` · `cnb-config` · `cnb-auth` · `cnb-git` · `cnb-tty` · `xtask`

**Crate 列表（当前，SDK-2 完成后）：** `cnb`（bin）· `cnb-cli` · `cnb-config` · `cnb-auth` · `cnb-git` · `cnb-tty` · `xtask` —— **`cnb-api` 已退役**，所有 HTTP 通过外部 crate `cnb-sdk`（即 crates.io 上的 `cnb`）发出。详见 [`docs/sdk-0.2.2-upgrade.md`](docs/sdk-0.2.2-upgrade.md) §6。

（M2 / M3 / M4 没有引入新 crate；`cnb-cli` 累计加入了 `cnb-git` + `indicatif` + `open` + `clap_complete` + `toml` 依赖。）

## 开发

> **想贡献代码？** 完整指南见 [`CONTRIBUTING.md`](CONTRIBUTING.md)
> （PR 流程、commit message 约定、测试要求、代码风格规则）。下面只是本地最小循环。

```bash
# 构建全部
cargo build --workspace

# 跑测试（单元 + 集成）
cargo test --workspace

# Lint
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings

# 同步上游 OpenAPI spec（写入 openapi/）
cargo xtask sync-openapi
```

### 内部 / 测试用环境变量

| 变量名             | 用途                                                         |
| ------------------ | ----------------------------------------------------------- |
| `CNB_API_BASE`     | 覆盖 base URL（默认 `https://api.cnb.cool`）。仅供基于 wiremock 的集成测试使用。 |
| `CNB_CONFIG_DIR`   | 覆盖配置目录（CI / 容器场景）。                              |
| `CNB_TOKEN`        | Bearer token（最高优先级）。                                  |
| `CNB_HOST`         | 默认 host（默认 `cnb.cool`）。                                |

## 许可

双许可：MIT 或 Apache-2.0。
