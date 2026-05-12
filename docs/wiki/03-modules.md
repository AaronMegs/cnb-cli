# 03 · 核心模块功能

按依赖序列，从 bin 进入到底层 crate。每节回答三件事：**职责** · **public API** · **关键不变式**。

---

## 3.1 `crates/cnb`（bin · 37 行）

### 职责

唯一的可执行 entry point。**只做三件事**：

1. `init_tracing(verbosity)` —— 用 `tracing_subscriber` 绑 stderr
2. `Cli::parse()` —— clap 解析 argv
3. `cnb_cli::run(cli).await` —— 跳进业务，捕获错误、打印、按 `exit_code()` 退出

### 关键不变式

- **零业务逻辑** —— 任何"如果 ... 就 ..."都属于 cnb-cli，便于测试时直接 `cnb_cli::run(cli)` 而不 fork 子进程
- **tracing 落 stderr** —— stdout 是脚本可信赖的数据流，绝不能被日志污染（`-v` / `-vv` 也只增 stderr 详细度）

---

## 3.2 `crates/cnb-cli`（lib · ~7300 行 · 业务主体）

### 模块拓扑

```
crates/cnb-cli/src/
├── lib.rs              # pub fn run(cli) → 18-arm dispatch
├── cli.rs              # clap Cli + Commands enum (18 variants)
├── context.rs          # Context（会话容器） · 369 行
├── error.rs            # CliError + exit_code()
├── http/               # SDK 不建模的 HTTP 兜底
│   ├── mod.rs
│   ├── passthrough.rs  # `cnb api …`
│   ├── uploads.rs      # `cnb issue --attach`
│   └── sensitive.rs    # 敏感 header redact 助手
└── commands/           # 18 个命令组
    ├── mod.rs
    ├── auth.rs         # 245 行
    ├── api.rs          # 204 行
    ├── repo.rs         # 758 行（最重）
    ├── issue.rs        # 808 行（最重）
    ├── label.rs
    ├── pr.rs           # 808 行
    ├── build.rs        # 459 行
    ├── workspace.rs    # 307 行
    ├── release.rs      # 567 行
    ├── registry.rs
    ├── mission.rs
    ├── org.rs          # 366 行
    ├── browse.rs
    ├── completion.rs
    ├── config.rs
    ├── alias.rs
    ├── update.rs
    └── search.rs       # 153 行（首个 typed SDK 消费者）
```

### `Context`（会话容器）—— 必懂

每个 `cnb` 子命令的入口签名都是 `async fn run(ctx: &mut Context, args: ...Args) -> Result<(), CliError>`。`Context` 持有：

| 字段 | 用途 | 懒构造？ |
|------|------|:----:|
| `host: String` | 当前 cnb 主机（默认 `cnb.cool`） | 立即从 `--hostname` / `CNB_HOST` 读 |
| `io: IoStreams` | stdout/stderr 是否 TTY、color mode | 立即 |
| `hosts_path: PathBuf` | `~/.config/cnb/hosts.toml` | 立即 |
| `keyring: Box<dyn KeyringBackend>` | 真实 OS keyring 或测试 InMemory | 立即（按 `CNB_KEYRING_BACKEND` env 选） |
| `sdk: Option<cnb_sdk::ApiClient>` | typed SDK client | **懒**（首次 `ctx.sdk()?` 时构造，含 token resolve） |
| `sdk_base_url: Option<String>` | wiremock 测试用 base URL override | 测试注入 |

**唯一的 token 拿取路径**：`Context::sdk()` → `cnb_auth::resolve_token(env > keyring > file)` → `cnb_sdk::ClientBuilder::token(...)`。我们**永远不**让 SDK 走它自己的 `CNB_TOKEN` env fallback，避免在 CI 等 env 隔离场景出现 surprise。

### `Context` 的 raw helpers

| 方法 | 用途 | 何时用 |
|------|------|------|
| `sdk()` | typed client 入口 | 默认选择 |
| `sdk_with_token(token)` | 一次性 client（用临时 token，例：`auth login` 验证用户输入的 token） | `auth login` / `auth status` |
| `sdk_raw_get(path)` | GET 一个 path → `serde_json::Value` | SDK DTO 与 wire 不一致时（如 `repo list` 的 `flags` 字段） |
| `sdk_raw_json(METHOD, path, body)` | 任意 method + JSON body → `Value` | SDK 没暴露的 endpoint 或 wire shape 漂移 |
| `sdk_raw_get_bytes(path)` | GET 一个 path → `Vec<u8>` | `release download` / `build logs` |
| `set_sdk_base_url(url)` | 测试 wiremock 注入 | 只在 `#[cfg(test)]` |

### 关键不变式

1. **`Context::sdk()` 是唯一允许的 typed-call 入口** —— 不要在命令里手写 `cnb_sdk::ApiClient::builder()...`
2. **`sdk_raw_*` 用于"SDK 不建模 / DTO 与 wire 漂移"两类场景** —— 用前先在源码注释里写清原因（典型例子：`commands/repo.rs::list` 里的 `Repos4UserBase.flags` 注释）
3. **不在命令里直接读 env** —— 所有 env 都通过 `Context` 中转（除了 clap 自动绑定的 `CNB_HOST` / `CNB_TOKEN`）

---

## 3.3 `crates/cnb-cli/src/http/`（兜底 HTTP · ~410 行）

### 为什么存在

cnb-sdk 0.2.2 的 typed 接口几乎覆盖所有业务，但**两类形态它不建模**：

| 形态 | 影响命令 | 模块 |
|------|--------|------|
| 任意 method + 任意 body + 拿 raw `(status, headers, body)` | `cnb api` 通用直通 | `http::passthrough` |
| `multipart/form-data` 文件流上传 | `cnb issue create --attach` / `cnb issue comment --attach` | `http::uploads` |

### 实现策略

- **复用 SDK 共享 reqwest**：`ctx.sdk()?.http().reqwest_client()` 拿到 `&reqwest::Client`，**不自己 `reqwest::Client::new()`**
- **复用 SDK URL builder**：`client.http().url(path)` 拼 base URL + percent-encode
- **不实现 retry**（passthrough）：`cnb api` 是 `gh api` 风格的逃生门，失败立即冒泡

### `http::passthrough` 主要 API

```rust
pub struct PassthroughResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub async fn request(
    ctx: &mut Context,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    extra_headers: &[(String, String)],
) -> Result<PassthroughResponse, CliError>;

pub fn into_error(resp: PassthroughResponse) -> CliError;
```

`into_error` 把 4xx/5xx 映射到 `CliError::Unauthorized` / `NotFound` / `RateLimited` / `ServerError`，**保留 DESIGN §12 的 exit code 契约**。

### `http::uploads` 主要 API

```rust
pub enum Scope<'a> {
    Repo(&'a str),
    IssueComment { repo: &'a str, number: u64 },
}

pub enum Kind { Image, Other }

pub struct Uploaded {
    pub url: String,
    pub kind: Kind,
    pub original_name: String,
}

pub async fn upload_one(
    ctx: &mut Context,
    scope: Scope<'_>,
    path: &Path,
    explicit_kind: Option<Kind>,
) -> Result<Uploaded, CliError>;
```

支持两阶段上传（POST 元数据 → POST 文件流），自动 `mime_guess` 探测类型，输出可直接拼到 markdown body。

### `http::sensitive`

```rust
pub fn is_sensitive(header_name: &str) -> bool;
```

22 个敏感 header（authorization / cookie / set-cookie / x-api-key / ...）。`cnb api -i` 打印响应 header 时调用，把敏感 header 值替换为 `***`。

---

## 3.4 `crates/cnb-auth`（540 行）

### 职责

token 的"拿到 + 验证 + 存储"。**纯逻辑、不依赖 cnb-cli**。

### 模块组织

| 文件 | 职责 |
|------|------|
| `error.rs` | `AuthError`（NotLoggedIn / NoUser / Backend / Io） |
| `keyring_backend.rs` | trait `KeyringBackend` + `RealKeyring`（OS）+ `InMemoryKeyring`（test） |
| `resolver.rs` | `resolve_token(host, user, kr, hosts_path) -> (String, TokenSource)` |
| `service.rs` | `AuthService::{login, logout, status, list_users, set_default_user}` |
| `lib.rs` | re-export + `KEYRING_SERVICE` 常量 + `ENV_TOKEN` 常量 |

### Token 三层降级（DESIGN §5）

```
1. CNB_TOKEN env            ← 最高优先级（CI 友好；不写盘）
   ↓ 没设
2. system keyring           ← 默认安装路径
   ↓ 拿不到
3. ~/.config/cnb/hosts.toml ← 兜底（只在用户拒绝 keyring 时用）
```

返回的 `TokenSource` 枚举携带"从哪儿拿到的"，让 `cnb auth status` 能告诉用户来源。

### 关键不变式

- **`KeyringBackend` 是 trait** —— 测试 `InMemoryKeyring` 替身让 CI 不依赖 OS keyring
- **`hosts.toml` 写盘走 `cnb_config::atomic_write`** —— 写时 `0600` mode + tmp file rename
- **`resolve_token` 是纯函数**（除了对 trait 和 path 的访问）—— 所有 env / 文件读都在 `resolver.rs` 一处，便于属性测试

---

## 3.5 `crates/cnb-config`（540 行）

### 职责

两个 TOML 文件 + 路径 + 原子写。

| 文件 | 内容 | 模块 |
|------|------|------|
| `~/.config/cnb/config.toml` | 用户偏好（editor / pager / color / aliases / output 默认） | `config.rs` · `Config` / `CoreConfig` / `OutputConfig` |
| `~/.config/cnb/hosts.toml` | auth state（default host / default user / 可选 fallback token） | `hosts.rs` · `Hosts` / `HostEntry` / `UserEntry` |

### 关键 API

```rust
// paths.rs
pub fn config_dir() -> Result<PathBuf, ConfigError>;
pub fn hosts_file() -> Result<PathBuf, ConfigError>;
pub fn config_file() -> Result<PathBuf, ConfigError>;

// atomic_write.rs
pub fn atomic_write(path: &Path, content: &[u8], mode: u32) -> Result<(), ConfigError>;
pub fn set_secure_permissions(path: &Path) -> Result<(), ConfigError>;

// hosts.rs
impl Hosts {
    pub fn load_from(path: &Path) -> Result<Self, ConfigError>;
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError>;
    pub fn default_user(&self, host: &str) -> Option<&str>;
    pub fn upsert(&mut self, host: &str, user: &str, entry: UserEntry);
    // ...
}

// config.rs
impl Config {
    pub fn load() -> Result<Self, ConfigError>;
    pub fn save(&self) -> Result<(), ConfigError>;
    // ...
}

pub const SCHEMA_VERSION: u32 = 1;
```

### 关键不变式

- **写盘必走 `atomic_write`** —— tmp file + fsync + rename，避免 ctrl-C 期间留下半文件
- **Unix 下 mode `0600`**，Windows 下信任默认 NTFS profile ACL（known-gaps #15 取舍）
- **`SCHEMA_VERSION = 1`** —— 文件首字段，未来升级时用于迁移逻辑

---

## 3.6 `crates/cnb-git`（190 行）

### 职责

把"我现在在哪个 cnb 仓库工作目录"翻译成 `OWNER/REPO[/SUBPATH]` slug。**不是 libgit2 包装**，只跑 git 子进程。

### 主要 API

```rust
// remote.rs
pub fn parse_remote_url(url: &str) -> Option<RepoSlug>;

pub struct RepoSlug {
    pub owner: String,
    pub repo: String,
    pub subpath: Option<String>,
}

impl RepoSlug {
    pub fn as_path(&self) -> String;  // "owner/repo" or "owner/repo/sub"
}

// git_cmd.rs
pub fn get_remote_url(remote: &str) -> Result<String, GitError>;
```

支持的 URL 形态：

- `https://cnb.cool/<owner>/<repo>(.git)?`
- `git@cnb.cool:<owner>/<repo>(.git)?`
- `https://cnb.cool/<owner>/<group>/.../<repo>` —— 子组路径
- 自托管 cnb 的别名 host（`--hostname` 配合）

### 关键不变式

- **不缓存** —— 每次 `Context::resolve_repo(None)` 都重新跑 `git remote get-url origin`，避免 user 切换 worktree 后命中过期 cache
- **不写 git 配置** —— 只读
- **fall-fast** —— 找不到 git 仓库立即 `Err(GitError::NotARepo)`，不沉默回退

---

## 3.7 `crates/cnb-tty`（420 行）

### 职责

输出格式化的"小语言"。**纯函数 / 无网络 / 无 IO**（除了 io_streams 探测 TTY）。

### 模块组织

| 文件 | 职责 |
|------|------|
| `io_streams.rs` | `IoStreams { stdin/stdout/stderr_is_tty }`，构造一次复用 |
| `color.rs` | `ColorMode::{Auto, Always, Never}` + `should_color(tty, mode)` |
| `table.rs` | `write_table(out, headers, rows, is_tty)` —— TTY 时用 `comfy-table`，pipe 时用 TSV |
| `json_out.rs` | `write_json(out, value, pretty)` —— pipe 时单行紧凑，TTY 时缩进 |
| `template.rs` | `apply(value, template_str)` —— `tinytemplate` 包装，把 `{{path}}` 风格替换 |
| `jq.rs` | `apply(value, expr)` —— `jaq-interpret` 跑 jq 表达式，返回 `Vec<Value>` |

### 关键不变式

- **`is_tty` 决定渲染策略** —— TTY 上人类友好（颜色 / 表格框线 / 缩进），pipe 上脚本友好（TSV / 单行 JSON / 无颜色）
- **任何输出都过 `cnb-tty`** —— commands 不直接 `println!("colored stuff")`
- **jq 用纯 Rust 实现**（`jaq-*`）—— 不依赖 system jq 二进制，跨平台可分发

---

## 3.8 外部依赖：`cnb-sdk`（即 crates.io 的 `cnb` 0.2.2）

### 它是什么

cnb 平台的官方 typed Rust SDK，由 OpenAPI spec 自动生成。**workspace 里 alias 为 `cnb-sdk`** 来避开和本 bin 同名（`cnb`）。

```toml
# Cargo.toml
cnb-sdk = { package = "cnb", version = "0.2",
            default-features = false,
            features = ["rustls-tls", "retry", "all-resources"] }
```

### 我们用它的哪些层

| 层 | 调用形态 | 例子 |
|----|--------|------|
| **Resource client** | `client.repositories()` / `client.issues()` / `client.pulls()` ... | 大多数命令 |
| **HttpInner（底层）** | `client.http().reqwest_client()` / `client.http().url(path)` | `crates/cnb-cli/src/http/` 的 passthrough + uploads |
| **ApiError** | `cnb_sdk::ApiError`（`#[from]` 进 `CliError::Sdk`） | 所有错误归一 |

### 已知 SDK 痛点（19 项 → 9 项已解决）

详见 [`docs/sdk-issues.md`](../sdk-issues.md) + [`docs/upstream-issues/`](../upstream-issues/)。本仓库的 workaround 通过 `Context::sdk_raw_*` + `crates/cnb-cli/src/http/` 兜底。

下一步推荐阅读：[04 命令清单与端点映射](./04-command-catalog.md)。
