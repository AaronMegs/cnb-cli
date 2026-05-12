# CNB CLI · 项目 Wiki

> **本 Wiki 是什么**：cnb-cli 仓库的"AI Wiki"风格知识库，按 [CodeWiki](https://codewiki.tencent.com/) 标准结构组织。每篇文档 self-contained、可独立阅读，目标读者是**接手项目的工程师**与**深度使用者**。
>
> **生成日期**：2026-05-12（锚定 commit `f3ae9f8`）。
> **与现有文档的关系**：本 Wiki **不取代** [`README.md`](../../README.md)（用户向）/ [`DESIGN.md`](../../DESIGN.md)（M0 史料档案）/ [`docs/src/`](../src/)（mdbook 用户手册）/ [`docs/known-gaps.md`](../known-gaps.md)（外部 open items 看板）/ [`docs/sdk-issues.md`](../sdk-issues.md)（SDK 上游进度），而是把它们的"工程视角导览"提炼到一处。

---

## 目录

| 编号 | 主题 | 适合 | 篇幅 |
|---:|------|------|----:|
| [01](./01-project-overview.md) | **项目概述** —— 是什么 / 解决什么问题 / 关键设计目标 / 不做什么 | 第一次接手 | 中 |
| [02](./02-architecture.md) | **系统架构** —— 6-crate workspace、依赖拓扑、HTTP 路径、Mermaid 总览图 | 想改架构前必读 | 中 |
| [03](./03-modules.md) | **核心模块功能** —— 每个 crate 一节，含 public API、内部组织、关键不变式 | 改模块前必读 | 长 |
| [04](./04-command-catalog.md) | **命令清单与端点映射** —— 18 个命令组 × 子动词 × HTTP 端点 × 退出码 | 加命令、查接口 | 长 |
| [05](./05-data-flows.md) | **核心数据流** —— `auth login` / `repo list` / `issue --attach` / `release upload` 等 5 个端到端时序图 | 调试、debug、性能分析 | 中 |
| [06](./06-developer-guide.md) | **二次开发指南** —— 如何加一个新命令、新 crate、新输出模式；测试与 CI 约定 | 贡献者必读 | 中 |

---

## 推荐阅读顺序

- **5 分钟快速浏览**：[01 项目概述](./01-project-overview.md) → [04 命令清单](./04-command-catalog.md) 看一眼即可对全貌有印象
- **接手项目（半天）**：01 → 02 → 03 → 06 这条线；遇到具体疑问再去 04 / 05 查
- **加新命令**：直接看 [06 开发指南](./06-developer-guide.md) §2 + [04 命令清单](./04-command-catalog.md) 找对标
- **排查线上问题**：[05 数据流](./05-data-flows.md) → 找对应的 crate → 跳到 [03 模块功能](./03-modules.md)

---

## 与项目其它文档的对照表

| 你想了解 | 本 Wiki | 其它来源（更详尽） |
|--------|------|------|
| 我是终端用户，怎么装、怎么用 | — | [README.md](../../README.md) / [docs/src/](../src/)（mdbook） |
| M0 时为什么这样设计 | — | [DESIGN.md](../../DESIGN.md) |
| 当前未完成项有哪些 | 简提，详见外链 | [docs/known-gaps.md](../known-gaps.md) |
| SDK 上游 19 项痛点的进度 | 简提 | [docs/sdk-issues.md](../sdk-issues.md) + [docs/upstream-issues/](../upstream-issues/) |
| cnb-sdk 0.2.2 升级做了什么 | 02 § "演化简史" | [docs/sdk-0.2.2-upgrade.md](../sdk-0.2.2-upgrade.md) |
| 版本历史 | — | [CHANGELOG.md](../../CHANGELOG.md) |

---

## 维护约定

- **不追代码细枝末节**：函数签名/字段表查源码即可，本 Wiki 只记**为什么**和**关系**
- **每次大重构后回顾一次**：commit message 提到 "wiki" 时务必 PR 改这里
- **新增 crate / 命令组 / 数据流**：分别更新 [03](./03-modules.md) / [04](./04-command-catalog.md) / [05](./05-data-flows.md) 的对应段
- **永远在 Wiki 入口（本文件）顶部更新"锚定 commit"**：让读者知道你看的是哪个版本的快照
