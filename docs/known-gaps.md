# Known gaps（外部依赖型 open item 汇总）

<!-- markdownlint-disable MD033 MD060 -->
<!-- MD033: `<date>` / `<commit>` 在本文档中是占位符模板、不是 HTML 标签。
     MD060: 长宽纵览表用 "aligned pipe" 反而破坏可读性，保持紧凑格式。 -->

> **目标读者**：想快速了解"cnb 仓库目前有哪些 open item、哪些是真的外部阻塞"的后续接手人，或做项目交接 / 路线评审用。
> **撰写时间**：2026-05-09。
> **锚定 commit**：`b785d35` + 本仓库 `[Unreleased]` 段全部 docs / `users::get_self` port 改动。
> **定位**：不是 "TODO list"，而是 "**这些事情在本仓库里做不了，请做决定**"。每一条都明确给出：**阻塞原因 / 影响范围 / 解除条件 / 建议负责人**。

---

## 0. 摘要

| # | 条目                                       | 类别         | 阻塞原因                              | 影响范围                                            | 解除条件                             |
|--:|--------------------------------------------|--------------|---------------------------------------|-----------------------------------------------------|--------------------------------------|
| 1 | ~~cnb-cli 公开镜像 URL~~ ✅                | 已解决       | canonical = `https://cnb.cool/aodoo/tools/cnb-cli` | —                                                  | —                                    |
| 2 | 5 份 Tier A upstream issue 实际发布        | 外部动作     | 依赖 #1 和上游 issue tracker         | 上游 SDK 不会知道 5 个阻塞级问题                   | #1 完成 + 逐一粘贴到上游 tracker     |
| 3 | Tier B / Tier C 合并 issue 发布            | 外部动作     | 依赖 #1                               | 上游 SDK 不会知道 14 个 polish/DTO 项             | #1 完成 + 两份合并稿贴到上游         |
| 4 | 上游 patch PR（SDK-I04/05/06/18）          | 外部动作     | 依赖 #1 + 上游仓库 contribution flow | 4 个最低争议改动无法落地                           | #1 完成 + 发 PR 到上游               |
| 5 | SDK-I14 · 非 JSON transport 修复           | 上游开发     | 等上游 SDK 发版                       | `release upload/download`、`build logs`、`cnb issue --attach` 四处 side-car reqwest | 上游暴露 `reqwest::Client` 或 bytes 方法 |
| 6 | SDK-I12 · `set_repo_visibility` wire 形态  | 服务端澄清   | 等 cnb 服务端团队确认 canonical shape | `cnb repo set-visibility` 集成测试按 SDK 形态 ship | 服务端团队回复；SDK 文档化           |
| 7 | SDK-I16 · `UpdateMembersRequest` wire 形态 | 服务端澄清   | 同 #6                                 | `cnb org member add/edit` 的 `--role` 字段语义     | 服务端团队回复                       |
| 8 | ~~`cnb-api::services::uploads` 彻底退休~~ ✅ | 已解决       | cnb 0.2.2 + 后续清理已完成           | —                                                  | —                                    |
| 9 | M5.2 · apt / yum / Docker image 分发       | 基础设施     | 需外部 apt repo / yum repo / registry | 对 Linux 发行版用户的安装路径覆盖不全             | 业主方 / ops 团队搭建 + 流水线对接   |
| 10| M6 · mdbook 文档站部署                     | 基础设施     | 需选定部署目标（cnb pages 等）       | 文档站只在本地构建，未上线                         | 选定托管并接入 release.yml           |
| 11| M6 · 外部 case study                       | 运营         | 需要至少 1 个外部用户案例            | v1.0 验收标准的最后一项                            | 找一个外部用户并收集反馈             |
| 12| DESIGN §16 #1 · OAuth Device Flow         | 上游 spec    | cnb OpenAPI 未暴露 OAuth 端点        | `auth login` 只能走 PAT，与 `gh` 有差距           | 服务端提供端点后再升级 `auth login`  |
| 13| DESIGN §16 #6 · registry 制品类型枚举     | 上游 spec    | spec 未明确 `type` 合法值            | `--type` 当前是自由字符串，无严格校验             | spec 补充枚举后加 clap `value_parser`|
| 14| DESIGN §16 #7 · cnb 错误码字典            | 上游文档     | 仅探知到 `errcode=5/16`              | `ApiError` 映射不全，部分 5xx 仅有 generic 提示    | 服务端发布完整码表 + 我方维护附录 B  |
| 15| Windows ACL 严格化                         | 取舍         | 需 `windows-sys` 依赖，scope 外      | 非 Unix 下 `hosts.toml` 仅靠 NTFS 默认 profile ACL 保护 | 若有 Windows-first 部署场景再做     |
| 16| `cnb pr list` 跨仓库视图（`/user/pulls`）  | 上游 API+SDK | 平台暂无 `/user/pulls` 端点          | `cnb pr list`（无 slug）只能 repo-scoped；与 `cnb issue list` 对称性缺失 | 服务端补端点 + SDK 暴露 typed 方法   |
| 17| 3 个 `unmaintained` advisory（`deny.toml` ignore） | 上游依赖 | `keyring 2.x → zbus 3.15` 锁住 `derivative`/`instant`；`indicatif 0.17` 锁住 `number_prefix` | CI audit 阶段需 ignore 三个 RUSTSEC ID 才能通过 | 升级到 `keyring 3.x`（带 zbus 4）+ `indicatif 0.18` 后即可移除 ignore |

共 **17 项**，全部无法在本仓库内单方面解决。下文按类别给出详情。

---

## 1. Upstream SDK 反馈链路（#1 – #4）

Phase 2 步骤 2.11 完成时累计出 19 个 SDK 痛点，已按 A/B/C 分级并产出完整草稿。镜像 URL 已经选定（`https://cnb.cool/aodoo/tools/cnb-cli`），#1 由此关闭；剩下 #2–#4 是把草稿粘到上游 issue tracker 的 social work。

### #1 ~~cnb-cli 公开 mirror URL 待选定~~ — ✅ Resolved

- **如何解决**：canonical 仓库地址定为 [`https://cnb.cool/aodoo/tools/cnb-cli`](https://cnb.cool/aodoo/tools/cnb-cli)（cnb.cool 上即开放可读，与 GitHub 镜像 [`AaronMegs/cnb-cli`](https://github.com/AaronMegs/cnb-cli) 双向同步）。`Cargo.toml` 的 `repository` 字段、`book.toml` 的 `git-repository-url` / `edit-url-template`、README / CHANGELOG / DESIGN / docs/* 内的 clone & link 全量改完，对应 commit 见 git 历史。
- **后续**：`docs/upstream-issues/*.md` 里那些"`https://…` 锚点"占位符可以按需替换为真实 commit URL（不再是阻塞 #2–#4 的硬条件 —— 上游可以从 cnb.cool / GitHub 任一镜像访问）。
- **历史保留**：本条留作 audit trail；下一次 known-gaps 维护可归档。

### #2 Tier A 5 份 issue 发布

- **现状**：草稿 5 份已就绪（`docs/upstream-issues/SDK-I03.md` / `SDK-I07.md` / `SDK-I09.md` / `SDK-I14.md` / `SDK-I15.md`），每份自包含（问题面 / 复现代码 / wiremock minimal test / 建议修复）。中文整合版见 [`SDK-反馈汇总.md`](./upstream-issues/SDK-反馈汇总.md) §3。
- **为什么阻塞**：依赖 #1。
- **解除条件**：#1 完成后，把 5 份 markdown 逐一粘贴到上游 `cnb` crate 的 issue tracker（或项目约定的 feedback channel）。
- **建议负责人**：任何了解项目的人。

### #3 Tier B / Tier C 合并 issue 发布

- **现状**：两份合并稿就绪：
  - `docs/upstream-issues/Tier-B.md` — DTO completeness bundle（6 子项：SDK-I01/I02/I08/I11/I13/I19），附建议合入顺序。
  - `docs/upstream-issues/Tier-C.md` — Polish & conventions meta-issue（8 子项，按 6 个 subgroup 分组）。
- **为什么阻塞**：依赖 #1。
- **解除条件**：#1 完成后，两份 markdown 分别作为一个上游 issue 贴出去。
- **建议负责人**：同 #2。

### #4 上游 patch PR · 最低争议 4 项

- **现状**：Tier C 文末标注了 4 个 "patch-ready" 项：
  - SDK-I04 · crates.io 改名为 `cnb-sdk` / `cnb-client`。
  - SDK-I05 · query/body struct 加 `#[non_exhaustive]`。
  - SDK-I06 · `repository` URL 公开可读或重定向到 mirror。
  - SDK-I18 · `set_pinned_repos(...)` 方法补上。
- **为什么阻塞**：依赖 #1（PR description 里需要引用 cnb-cli 的锚点），且需要走上游仓库的 contribution flow。
- **解除条件**：#1 完成 + 上游 SDK 仓库的 `CONTRIBUTING.md` 可读 + 作者同意直接 PR。
- **建议负责人**：作者或熟悉 SDK 代码的人。

---

## 2. 上游 SDK 修复（#5 – #8, #16）

这些不是"我们写文档就能结束"的问题，等上游发版。

### #5 SDK-I14 · 非 JSON transport（**最大上游阻塞点**）

- **现状**：SDK 的 `HttpInner::{execute, execute_with_body}` 全部 JSON-only。我们用 3 个 side-car `reqwest::Client::new()` 绕开（细节见 `docs/upstream-issues/SDK-I14.md`）。
- **直接阻塞的 cnb-cli 行为**：
  1. `cnb release upload` phase 2（`PUT <pre-signed url>` 文件流）
  2. `cnb release download`（302 → bytes）
  3. `cnb build logs`（`text/plain` 日志流）
  4. `cnb issue create --attach` / `cnb issue comment --attach`（两阶段附件上传，`cnb-api::services::uploads` 存在的唯一理由）
- **解除条件**（上游方案任一）：
  - 方案 A · `pub fn HttpInner::reqwest_client(&self) -> &reqwest::Client`
  - 方案 B · `execute_with_body` 接受任意 `reqwest::Body` + 新增 `execute_bytes`
  - 方案 C · 三个 bytes 端点建模为 first-class typed method
- **我们偏好**：A（最小补丁）。
- **建议负责人**：上游 SDK 作者。我们可以提供 PR，但先等作者回应方向。

### #6 SDK-I12 · `set_repo_visibility` canonical wire shape

- **现状**：SDK 用 `?visibility=public` query string；原 cnb-api facade 用 `{visibility_level: 0}` JSON body。两种形态我们都没有 wire-level 集成证据。cnb-cli 的 wiremock 测试按 SDK 形态写，能跑；真实服务端的行为未验证。
- **为什么阻塞**：这是服务端问题，不是 SDK bug。需要 cnb 服务端团队明示 canonical 形状。
- **如果服务端选 body 形态**：本条升级为 **blocker**，我们得在 CLI 里用 `Context::sdk_raw_json(POST, …, body)` 绕过 typed 调用；SDK 也应该同步改成 body。
- **解除条件**：服务端团队回复 + SDK 文档在方法上明示。
- **建议负责人**：与 cnb 服务端团队对接。

### #7 SDK-I16 · `UpdateMembersRequest` canonical wire shape

- **现状**：SDK 用 `{access_level, is_outside_collaborator}`；原 facade 用 `{username, role}`（POST）/`{role}`（PUT）。与 #6 完全同构的服务端澄清问题。
- **临时表现**：`cnb org member add/edit --role X` 把 X 直接当 `access_level` 塞过去；响应展示侧从 `access_level` 字段读，丢了原 facade 对 legacy `role` key 的容忍。
- **解除条件**：服务端回复 canonical 形态；SDK 要么文档化，要么加 `#[serde(alias = "role")]` 双向容忍。
- **建议负责人**：与 cnb 服务端团队对接。

### #8 ~~`cnb-api::services::uploads` 彻底退休~~ — ✅ Resolved

- **如何解决**：cnb 0.2.2 把 `HttpInner::reqwest_client()` 公开
  （SDK-I14 落地）+ 后续清理把 `uploads.rs` 移植成
  `cnb-cli::http::uploads`，直接用 SDK 共享 reqwest client 发送
  multipart POST。整个 `crates/cnb-api/` 目录已从 workspace 删除，
  `cnb-cli` 现在直接依赖 `cnb-sdk` 一个 HTTP 入口。
- **同时收尾**：`cnb api` raw passthrough 也搬到了
  `cnb-cli::http::passthrough`，`Context::api()` / `Client` 字段移除，
  `CliError::Api(cnb_api::ApiError)` variant 移除（拆成 explicit
  Unauthorized / NotFound / RateLimited / ServerError，DESIGN §12
  退出码映射保留）。
- **历史保留**：本条留作 audit trail；从下一次 known-gaps 维护开始
  可以归档（移到一个"resolved-archive"段落或直接删）。

### #16 `cnb pr list` 跨仓库视图（与 `cnb issue list` 对称）

- **现状**（commit `666ba20`，2026-05-12）：`cnb issue list` 已经把
  默认行为改成"跨用户所有仓库的相关 issue"，调用上游
  `GET /user/issues`（`ListUserIssues`），表加 `REPO` 列；显式传
  slug 时退化为单仓库 `GET /{slug}/-/issues`（`ListIssues`）。
  **`cnb pr list` 想做完全对称的事，但平台没有 `/user/pulls`
  端点**——已在 2026-05-12 实测：`/user/pulls`、`/user/pull-requests`、
  `/user/prs`、`/search/pulls`、`/search/issues` **全部返回 404 not
  found**。所以 `cnb pr list`（无 slug 时）目前还会走 `resolve_repo`
  → 当前 git remote → repo-scoped，且空表 hint 里明确告知用户"平台
  暂不支持跨仓库 PR 视图"。
- **目标形态**：一旦平台 + SDK 双端支持，按 `cnb issue list` 同款重构：
  - 默认（无 slug）→ 跨仓库 PR 视图，表加 `REPO` 列
  - 显式传 slug → repo-scoped（保留现有 `ListPulls` 路径）
  - 同步删除 `cnb pr list` 的"不再从 git remote 推断"footgun
    （和 issue 一致）
- **解除条件**（**任一上游路径**）：
  1. **首选**：cnb 平台新增 `GET /user/pulls`（或同义端点，shape 与
     `/user/issues` 对齐：返回带 `repo.path` 的扁平列表）+ cnb-sdk
     新版本暴露 `client.pulls().list_user_pulls(&q)` typed 方法。
  2. **次选**：cnb 平台暴露 `GET /search/pulls`（或 `/search/issues`
     带 `type=pull` 过滤）作为搜索路径，CLI 走 `Context::sdk_raw_get`
     模式拼装。
- **影响范围**：用户体验缺口（与 issue 不对称），但**不阻塞 v1.0
  发布**——repo-scoped PR 列表已可用。
- **关联代码锚点**：
  - `crates/cnb-cli/src/commands/pr.rs` — `ListArgs.repo` + `list()`
    主体 + 空表 hint（hint 里已留 forward-looking 文案）。
  - `crates/cnb-cli/src/commands/issue.rs` — 对照实现
    （`ListArgs.repo` 文档段 + `cross_repo` 分支），未来 PR 改造
    可直接套用同一模板。
- **建议负责人**：作者 / 跟 cnb 平台团队对接（端点） + 跟上游
  cnb-sdk 维护者对接（typed 方法）。条件齐备后 CLI 侧改造预计
  半天工作量（含 wiremock 集成测试）。
- **跟踪触发器**：每次 cnb-sdk 升级（看 `cnb_sdk::pulls` 是否新增
  `list_user_pulls` 同名方法）；每次平台 OpenAPI spec 更新时 grep
  `/user/pulls`。

---

## 3. 发行 & 基础设施（#9 – #11）

M5.2 / M6 里的"partial"实际上都卡在需要外部基础设施或运营动作。

### #9 M5.2 · apt / yum / Docker image 分发

- **现状**：`release.yml` 已生成 tarball + cosign 签名 + Homebrew/Scoop manifest 模板；apt repo、yum repo、Docker image 三条路径**未接入**。
- **需要的外部资源**：
  - apt · 一个签名公钥 + 可写的 repo server（或 Cloudflare R2 / GCS bucket + reprepro 工具链）。
  - yum · 类似的 repo server + `createrepo_c`。
  - Docker · 一个 container registry（Docker Hub / ghcr / cnb.cool 自带）+ 凭证注入到 release.yml。
- **解除条件**：业主方决定托管位置 → 把凭证写进 GitHub Actions secrets（或 cnb.cool pipelines secrets）→ release.yml 加三条 job。
- **建议负责人**：项目运营方。代码侧工作量不大（~半天）。

### #10 M6 · mdbook 文档站部署

- **现状**：`docs/book.toml` + 19 篇 markdown + SUMMARY 已就位，`mdbook serve` 本地能看；线上地址未定。
- **候选部署目标**：cnb pages（若 cnb.cool 支持）/ GitHub Pages / Cloudflare Pages / 自建。
- **解除条件**：选定托管 → release.yml 新增 `docs-deploy` job。
- **建议负责人**：同 #9。

### #11 M6 · 外部 case study

- **现状**：DESIGN §1.3 "v0.1 MVP 验收口径" 没直接要求这项；但 §15 M6 的 "验收" 列了 "至少 1 个外部用户案例"。
- **阻塞原因**：需要真实用户。
- **解除条件**：找一个愿意写几段反馈的外部用户（比如内部其它团队）。
- **建议负责人**：项目运营方。
- **非硬阻塞**：可以把这条从 v1.0 验收移到 v1.1 "grow the community" 线。

---

## 4. spec / 服务端长期不确定性（#12 – #14）

DESIGN §16 "风险与未决事项" 里一直没关掉的三项。现状基本是"知道有坑，暂时不填"。

### #12 OAuth Device Flow（DESIGN §16 #1）

- **现状**：OpenAPI spec 未暴露 OAuth 端点。`cnb auth login` 强制 PAT。
- **对用户体验的影响**：与 `gh auth login` 的 "打开浏览器 → 授权 → 自动拿 token" 有明显差距；CI 场景不受影响（`CNB_TOKEN` 环境变量）。
- **解除条件**：cnb 平台上线 OAuth Device Flow 端点 → 我们在 `cnb auth login` 加 `--web` 或默认分支。
- **建议负责人**：作者（跟 cnb 平台团队沟通）。

### #13 registry 制品类型枚举（DESIGN §16 #6）

- **现状**：`cnb registry` 所有接受 `--type` 的子命令都把它当自由字符串传下去；不在客户端做枚举校验。
- **风险**：用户拼错（`docker` 写成 `dockr`）会直接 404 才发现。
- **解除条件**：spec 补充合法值枚举 → clap `value_parser` 换成显式白名单 + 错别字提示。
- **建议负责人**：作者（跟 cnb 平台团队沟通 spec 补充）。

### #14 cnb 错误码字典（DESIGN §16 #7）

- **现状**：`cnb-api/src/error.rs` 的 `ApiError` 目前只识别 `errcode=5`（NotFound）/ `errcode=16`（Unauthorized），其余服务端错误都落到 `ApiError::Api { http_status, message, … }`，对用户的提示就是一句原始 message。
- **对用户体验的影响**：不致命（HTTP status + message 仍然能看），但对脚本化用户来说不方便（没法按 `errcode` switch）。
- **解除条件**：cnb 平台发布完整错误码表 → 我们在 `ApiError` 加分支 + 更新 `docs/src/advanced/error-codes.md`（对应 DESIGN 附录 B）。
- **建议负责人**：日常开发中遇到新错误码时持续补充；与作者配合向平台团队索取完整字典。

---

## 5. 平台取舍（#15）

### #15 Windows ACL 严格化

- **现状**：`cnb-config::atomic_write::set_secure_permissions` 在非 Unix 下直接 `Ok(())`，依赖 NTFS 默认的 per-user profile ACL（`%APPDATA%\cnb\hosts.toml` 天然只能自己读写）。Unix 下强制 `0600`。
- **不是 TODO，是取舍**：
  1. 项目主要目标平台是 Linux/macOS（内部开发者场景）。
  2. 显式 `windows-sys` DACL 重写会拉依赖、增加 binary size、增加跨编译复杂度。
  3. 现有方案对"同机器非管理员无法读取他人 hosts.toml"这一核心威胁模型已经够用。
- **解除条件**：如果出现以下任一情况：
  - 项目要在非默认 `%APPDATA%` 路径写 token（例如共享开发机、CI runner 的通用账户）。
  - Windows 成为"first-class 目标平台"（例如要做 Scoop bucket 以外的 Windows 分发渠道）。
- **建议负责人**：若真要做，Windows 熟手（引入 `windows` crate + DACL API）。
- **源码注释**：`crates/cnb-config/src/atomic_write.rs:84` 已更新为具体说明（不再是 `TODO(M5)`）。

---

## 6. 非阻塞但应关注的小事项

以下**不属于 open item**（都能在本仓库内完成或不在验收清单上），仅列出以免被误判为 gap：

1. **cnb-cli 自身 CHANGELOG 条目日渐变长** —— 可以在发 `v0.4.0-alpha.2` 时整体归档到 `CHANGELOG.md` 的一个小节下。
2. **19 个 SDK issue 对应的 wiremock 测试已覆盖** —— 集成测试套（`crates/cnb/tests/*.rs`）对每一个 workaround 都有 pinning test，未来 SDK 修复后测试会自动验证行为一致。
3. **`docs/sdk-issues.md` 里的 "Resolved issues" 章节目前为空** —— 一旦上游开始修复，这里会逐条迁移，是项目"健康度雷达"的入口。

---

## 附录 · 维护约定

- 本文档**每次 docs-only 归档**时都应复核一次（建议结合 `CHANGELOG.md` 的 Unreleased → Released 切换时机）。
- 每解除一项，在摘要表里把状态从 "❌ Open" 变更为 "✅ Resolved（<date> / <commit>）"，并保留条目做 history。
- 新增 gap 时遵循同一模板：**现状 / 阻塞原因 / 影响范围 / 解除条件 / 建议负责人**。
