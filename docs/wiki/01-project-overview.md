# 01 · 项目概述

> **一句话**：`cnb` 是 [CNB（CloudNative Build, cnb.cool）](https://cnb.cool) 平台的官方风格命令行工具，用 Rust 实现，命令模型对齐 GitHub `gh` CLI。

---

## 1.1 它解决什么问题

| 用户场景 | 没有 cnb 时 | 有 cnb 时 |
|--------|-----------|---------|
| 在终端创建 / 浏览 / 评论 issue 与 PR | 必须开浏览器登录 cnb.cool | `cnb issue create -t "标题" -b "正文"` |
| 触发流水线 / 看构建日志 | 同上 + 跨多页跳转 | `cnb build run` / `cnb build logs <sn>` |
| 上传 release 二进制（含跨平台多文件） | 浏览器拖拽，慢且无法脚本化 | `cnb release upload v1.0 ./dist/*` |
| 在 CI/CD 里调用 cnb API | 手写 curl + token 注入 + 解析 JSON | `cnb api /endpoint --jq '.[].field'` |
| 跨仓库看"我的"issue / 待办 | 无内置入口 | `cnb issue list`（默认即跨仓库） |

**核心价值**：把"在 cnb.cool 上能做的所有日常运维操作"做成可脚本化、可组合的本地命令，与现有的 `gh` / `kubectl` / `docker` 等工具同质化体验。

---

## 1.2 设计目标（v0.1 ~ v1.0）

按重要度排：

1. **gh-shaped UX** —— 资源-动作命令模型（`cnb <resource> <verb>`），全局 `--json` / `--jq` / `--template`，与 GitHub `gh` 用户的肌肉记忆兼容
2. **可脚本化** —— 任何输出都有 JSON 模式；任何错误都有稳定的 exit code（DESIGN §12）；任何隐式默认都能被 env 或 `--flag` 覆盖
3. **可信** —— token 三层降级（`CNB_TOKEN` env > 系统 keyring > `hosts.toml`），文件 `0600` mode；敏感 header 在日志中 redact
4. **跨平台** —— Linux / macOS first-class；Windows best-effort（Scoop 安装 + cmd 兼容，但 ACL 严格化是 known gap，见 `docs/known-gaps.md` #15）
5. **轻依赖、快启动** —— 单二进制，冷启动 < 50ms（无 GC 的 Rust + 懒加载 SDK client）

---

## 1.3 非目标（明确不做）

| 非目标 | 为什么不做 |
|------|----------|
| **替代 git** | git 已经是事实标准；cnb 只做 cnb 平台**特有**的资源管理 |
| **GraphQL 客户端** | cnb 平台目前只有 REST，OpenAPI 完整覆盖 |
| **后台守护进程 / TUI** | 单次命令式工具是 gh 的成功要素，cnb 同等取舍 |
| **本地缓存层** | 一致性 > 速度；token 验证、配置读、git remote 解析都是 ms 级，加缓存得不偿失 |
| **复杂插件系统** | 用 shell alias + `cnb api` passthrough 已能覆盖 90% 扩展需求 |

---

## 1.4 关键里程碑（实施视角）

| 阶段 | 状态 | 主要交付 |
|------|----|--------|
| **M0** 设计冻结 | ✅ | DESIGN.md（架构、命令清单、端点映射） |
| **M1** 骨架 + auth + `cnb api` | ✅ | workspace、token resolver、generic passthrough |
| **M2** 核心资源 CRUD | ✅ | repo / issue / label / pr |
| **M3** 自动化与发布 | ✅ | build / workspace / release |
| **M4** 平台扩展 | ✅ | registry / mission / org / browse / completion / config / alias |
| **M5.0** 自动更新检查 + release 流水线 | ✅ | `cnb update`、Homebrew/Scoop manifest 模板 |
| **M5.1** —— | ✅ | （并入 M5.0） |
| **SDK-1** typed SDK 切入 | ✅ | `cnb search` 作为首个消费者 |
| **SDK-2** 全面切到 cnb-sdk | ✅ | 12 个 service facade 全部从 cnb-api 迁出 |
| **SDK-2 follow-up** cnb 0.2.2 升级 | ✅ | 9/19 痛点解决 + side-car reqwest 收编 |
| **cnb-api crate 退役** | ✅ | workspace 8 → 6（commit `9547335`） |
| **M5.2** apt / yum / Docker 分发 | ⏸️ | 等基础设施决策（known-gaps #9） |
| **M6** mdbook 部署 + 外部 case study + v1.0 | ⏸️ | 等托管目标 + 外部用户（known-gaps #10/#11） |

详见 [`README.md`](../../README.md) 顶部的 milestone status 表，或 [`docs/known-gaps.md`](../known-gaps.md) 看 16 项外部依赖型 open item。

---

## 1.5 技术栈一览

| 层 | 选型 | 备注 |
|----|------|------|
| **语言** | Rust 1.86 (edition 2021) | rust-toolchain.toml 锁定 |
| **CLI 框架** | `clap` 4 (derive + env + wrap_help) | 子命令、补全、man page 一站 |
| **异步运行时** | `tokio` 1（multi-thread + fs + process + signal + io-util + sync + time） | `#[tokio::main]` 在 bin 入口 |
| **HTTP 客户端** | `cnb-sdk`（外部 crate `cnb` 0.2.2，alias 避撞）+ `reqwest` 0.12 | 详见 [02 § HTTP 路径](./02-architecture.md) |
| **TLS** | `rustls`（不依赖 OpenSSL） | 跨平台静态编译友好 |
| **JSON / TOML** | `serde` 1 / `serde_json` / `toml` 0.8 | 配置文件 = TOML，输出 = JSON |
| **凭据存储** | `keyring` 2（macOS Keychain / Linux Secret Service / Windows Credential Manager） | trait 化便于测试 mock |
| **TTY 渲染** | `comfy-table` / `owo-colors` / `is-terminal` | 自动 disable color when piped |
| **JQ 过滤** | `jaq-interpret` + `jaq-parse` + `jaq-core` + `jaq-std` | 纯 Rust，避免 system jq 依赖 |
| **Markdown 渲染**（`issue view` 等） | `termimad` 0.30 | issue body 在终端漂亮显示 |
| **进度条 / 文件操作** | `indicatif` / `tokio-util` / `mime_guess` / `fs2` / `tempfile` | release upload / issue --attach 用到 |
| **错误模型** | `thiserror`（库层）+ `anyhow`（bin 顶层） | DESIGN §12 |

---

## 1.6 一眼看代码规模

| Crate | 行数 | 角色 |
|-------|----:|------|
| `cnb` (bin) | 37 | 入口，仅 init tracing + parse argv + 调度 |
| `cnb-cli` (lib) | **~7300** | 业务主体（18 个命令组 + context + http） |
| `cnb-auth` | 540 | token resolver + AuthService + KeyringBackend |
| `cnb-config` | 540 | hosts.toml / config.toml + atomic_write + paths |
| `cnb-git` | 190 | git remote 解析（无 libgit2，调子进程） |
| `cnb-tty` | 420 | jq / template / table / color / json_out |

总计约 **9000 行业务代码**，180 个测试（24 ok rows × 各 crate / 集成）。

下一步推荐阅读：[02 系统架构](./02-architecture.md)。
