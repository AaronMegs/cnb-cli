# cnb pipeline (`.cnb.yml`) 配置说明

> 项目同时维护两套 CI 配置：
>
> - **`.github/workflows/{ci,docs,release}.yml`** —— GitHub Actions（GitHub mirror 上跑）
> - **`.cnb.yml`** —— cnb 平台（cnb.cool 主仓库上跑）
>
> 两套**功能等价但不强求字段一一对应**，因为 cnb 平台和 GitHub Actions 的能力模型不同。本文档解释 `.cnb.yml` 的设计取舍 + 与 GitHub workflow 的对照表。

---

## 1. 为什么不能直接复制 GitHub workflow

| 维度 | GitHub Actions | cnb pipeline | 翻译策略 |
|------|--------------|------------|---------|
| **配置文件** | `.github/workflows/*.yml`（多文件） | `.cnb.yml`（单文件，多顶层分支 → 事件 → pipeline 数组） | 把 ci/docs/release 三个 workflow 合并到一个 `.cnb.yml` 的不同事件下 |
| **runner OS** | `ubuntu-latest` / `macos-latest` / `windows-latest` 原生 | **只有 Linux amd64 / arm64**（[文档](https://docs.cnb.cool/zh/build/build-node.html)） | macOS / Windows binary **必须交叉编译**（`cross` + `cargo-zigbuild` + `mingw-w64`） |
| **缓存** | `actions/cache` / `Swatinem/rust-cache` action | `docker.volumes` `copy-on-write` 类型 | 挂 `~/.cargo/registry` + `./target` 即可 |
| **secret** | repo settings → Secrets 注入 env | imports 引用「密钥仓库」中的 yml/json | 本配置当前不依赖密钥；cosign keyless 改成 sha256 |
| **artifact 跨 job** | `actions/upload-artifact` + `actions/download-artifact` | 同一 pipeline 内串行 stages 共享工作区，跨 pipeline 需 push 到制品库 / 仓库 | 把 release 的 5 个 build job 合并到一个 pipeline 的串行 stages，省去 artifact 上传 |
| **release 上传** | `softprops/action-gh-release` | `git:release` 内置任务（**不支持附件**）+ `cnbcool/attachments` 插件 | 两步：先 `git:release` 创建 release，再 `cnbcool/attachments` 上传 binary archive |
| **触发条件** | `on.push.branches` / `paths` / `tags` | 顶层 = 触发分支，`push:` / `pull_request:` / `tag_push:` 嵌套；`paths` ↔ `ifModify` glob | 直接翻译 |
| **并发取消** | `concurrency: group: ... cancel-in-progress: true` | `lock: { key: ..., cancel-in-progress: true }` | 同义 |
| **Sigstore keyless** | 直接用 `sigstore/cosign-installer@v3` + GitHub OIDC token | cnb 上没有 GitHub OIDC，需要外部 cosign key | 当前只产 sha256，cosign 留 TODO |

---

## 2. `.cnb.yml` 结构总览

```text
.cnb.yml
├── main:                          # 仅 main 分支触发
│   ├── push:                      # main 推送代码 → CI 全套
│   │   ├── pipeline lint          # rustfmt + clippy
│   │   ├── pipeline test-linux-amd64
│   │   ├── pipeline test-linux-arm64    # 验证交叉构建链路
│   │   ├── pipeline audit         # cargo-deny + cargo-audit
│   │   └── pipeline docs          # mdbook build（ifModify=docs/**）
│   └── pull_request:              # PR 进 main → CI 子集
│       ├── pipeline lint          # 带 lock cancel-in-progress
│       ├── pipeline test
│       └── pipeline docs          # ifModify=docs/**
└── $:                             # 兜底分支（任意 tag 都触发）
    └── tag_push:                  # 推 tag v* → release 流程
        └── pipeline release
            ├── stage 1  render man + completions（xtask gen-dist）
            ├── stage 2  install cross
            ├── stage 3  build linux x86_64
            ├── stage 4  build linux aarch64       (cross)
            ├── stage 5  build macos x86_64        (cargo-zigbuild + zig)
            ├── stage 6  build macos aarch64       (cargo-zigbuild + zig)
            ├── stage 7  build windows x86_64      (cross + mingw-w64)
            ├── stage 8  collect + sha256
            ├── stage 9  cosign sign（TODO）
            ├── stage 10 git:release 创建 release
            └── stage 11 cnbcool/attachments 上传所有 archive
```

**关键不变式**：

- `runner.tags` 决定平台：`cnb:arch:amd64` 或 `cnb:arch:arm64:v8`
- `docker.image` 决定语言运行时：用 `rust:1.86` / `rust:1.86-bookworm` 与 `rust-toolchain.toml` 对齐
- `docker.volumes` 用 `copy-on-write` 缓存 cargo registry / target，加速二次构建
- `lock` + `cancel-in-progress: true` 让同 PR 多次 push 自动取消上一次未完成的 pipeline，节省 runner

---

## 3. 与 `.github/workflows/*.yml` 的逐项对照

### 3.1 `ci.yml` → `.cnb.yml main:push`

| GitHub job | cnb pipeline | 行为差异 |
|-----------|------------|--------|
| `test (matrix os × toolchain)` | `test-linux-amd64` + `test-linux-arm64` | GitHub 跑 3 OS × 2 toolchain = 6 矩阵；cnb 只跑 Linux amd64 + arm64（无 macOS / Windows runner），且只 pin 1.86 toolchain（与 rust-toolchain.toml 一致）。覆盖度降低，但每条 release 的 binary 在交叉编译时仍会被验证（rustc 同样会跑 type check + 编译） |
| `lint (fmt + clippy)` | `lint` | 完全等价 |
| `audit (cargo-deny + cargo-audit)` | `audit` | 等价；cargo-audit 同样设为 informational（`allowFailure: true`） |

### 3.2 `docs.yml` → `.cnb.yml main:push pipeline docs`

| GitHub | cnb |
|--------|-----|
| `peaceiris/actions-mdbook@v2` | `cargo install mdbook --locked --version "0.4.40"` |
| `paths: docs/**` | `ifModify: ["docs/**", ".cnb.yml"]` |
| `actions/upload-artifact: cnb-handbook` | 当前只 `ls` 出 HTML 数量做 smoke。要做版本化 artifact，需走 cnb 制品库（Docker registry）或 cnb pages（已规划，known-gaps #10） |

### 3.3 `release.yml` → `.cnb.yml $:tag_push pipeline release`

| GitHub job | cnb stage | 差异 |
|-----------|----------|------|
| `assets: render man + completions` | stage 1 `render man + completions` | 等价 |
| `build: matrix x 5 targets` | stage 3-7（5 个串行 build stage） | GitHub 5 个 job 并行，cnb 串行（同 pipeline 共享工作区）。耗时增加但省去跨 job artifact 传输 |
| `actions/upload-artifact` | （省略）| cnb 串行模式不需要 |
| `softprops/action-gh-release` | stage 10 `git:release` + stage 11 `cnbcool/attachments` | cnb 平台的 release 创建和附件上传分两个任务（详见下方 §4） |
| `sigstore/cosign-installer + sign-blob keyless` | stage 9（TODO） | cnb 没有 GitHub OIDC，只生成 sha256 |

---

## 4. cnb release 上传的两步走

cnb 平台的 release 模型与 GitHub 不同：

1. **`git:release` 内置任务**：创建（或叠加更新）一个 Release 对象，挂在某个 tag 上。**不支持附件**（[文档明确](https://docs.cnb.cool/zh/build/internal-steps.html#git-release)）。
2. **`cnbcool/attachments` 插件**：上传 binary archive、sha256 sidecar、SHA256SUMS 等文件到指定 tag 的 Release 上。

`.cnb.yml` 把这两步串在一起：

```yaml
- name: create release
  type: git:release
  options:
    title: ${VERSION_TAG}
    descriptionFromFile: CHANGELOG.md
    latest: true

- name: upload assets to release
  image: cnbcool/attachments
  settings:
    target: dist
    attachments:
      - "dist/cnb-*-*.tar.gz"
      - "dist/cnb-*-*.zip"
      - "dist/cnb-*-*.sha256"
      - "dist/SHA256SUMS.txt"
    tag: ${CNB_BRANCH}
```

**等价的 GitHub Actions 是**：

```yaml
- uses: softprops/action-gh-release@v2
  with:
    files: release-assets/*
```

---

## 5. cosign 签名的当前状态（known limitation）

GitHub `release.yml` 用 sigstore keyless 签名（依赖 GitHub OIDC `id-token` permission），verifier 通过 `--certificate-identity-regexp` 验证签名来自 `https://github.com/<org>/<repo>` 的 Actions 上下文。

cnb 平台目前**没有等价的 OIDC token 提供方**（根据 [docs.cnb.cool](https://docs.cnb.cool/) 的公开文档）。两条 forward path：

- **A. 严格 cosign（推荐用于正式发布）**：
  - 在 cnb 仓库中放一个密钥仓库文件 `cosign.yml`：
    ```yaml
    COSIGN_PRIVATE_KEY: |
      -----BEGIN ENCRYPTED COSIGN PRIVATE KEY-----
      ...
      -----END ENCRYPTED COSIGN PRIVATE KEY-----
    COSIGN_PASSWORD: <your-password>
    ```
  - 在 release pipeline 加：
    ```yaml
    - imports: https://cnb.cool/<your-org>/secrets/-/blob/main/cosign.yml
    ```
  - cosign 命令改用 `--key env://COSIGN_PRIVATE_KEY` + `COSIGN_PASSWORD`：
    ```yaml
    - name: cosign sign
      script: |
        for f in dist/*.tar.gz dist/*.zip; do
          cosign sign-blob --yes \
            --key env://COSIGN_PRIVATE_KEY \
            --output-signature "$f.sig" \
            "$f"
        done
    ```

- **B. 当前选择 SHA256 校验和**：每个 archive 旁附 `*.sha256`，且在 dist/ 根生成 `SHA256SUMS.txt` 一键校验。下载用户用 `shasum -c SHA256SUMS.txt` 验证完整性（不验证身份，但能挡掉传输损坏 / 被替换文件的多数场景）。

详细对照 + 决策依据见 [`docs/known-gaps.md`](known-gaps.md)（计划登记为 #17，等首次 cnb release 时再加）。

---

## 6. 本地预演

cnb 平台上提交 `.cnb.yml` 修改后会自动跑流水线，但本地也可以做最小验证：

```bash
# 1. yaml 语法检查（避免 cnb 平台拒绝）
python3 -c 'import yaml; yaml.safe_load(open(".cnb.yml"))'

# 2. 打包脚本本地试跑（产出在 dist/ 下）
cargo build --release --bin cnb
mkdir -p dist
cp target/release/cnb dist/
BIN=cnb VERSION_TAG=v0.0.0-local bash scripts/cnb-pack.sh \
  $(rustc -vV | grep host | awk '{print $2}') ""
ls dist/
```

---

## 7. 维护 checklist

修改 `.cnb.yml` 时记得同步：

- [ ] 如果改了 `rust:1.86` 镜像版本 → `rust-toolchain.toml` + `.github/workflows/*.yml::toolchain` 也要改
- [ ] 如果加新 release target → `dist-templates/{homebrew,scoop}/*.tmpl` 也要加；`scripts/install.sh` 的 fetch 链接也要改
- [ ] 如果改了 mdbook 版本 → `.github/workflows/docs.yml::mdbook-version` 同步
- [ ] 如果改了 cargo-deny / cargo-audit 版本 → `.github/workflows/ci.yml::audit` 同步
- [ ] release pipeline 改动后，**手动推一个测试 tag**（如 `v0.0.0-test`）验证 binary 产出正确 + cosign / attachments 上传成功
- [ ] 同步更新本文档的对照表（§3）
