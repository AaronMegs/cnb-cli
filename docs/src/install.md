# 安装

## 一行安装脚本（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh | bash
```

环境变量 / 标志：

* `CNB_VERSION` 或 `--version vX.Y.Z`：指定版本（默认装最新 release）。
* `CNB_PREFIX` 或 `--prefix /path`：安装目录（默认 `$HOME/.local/bin`，若 `/usr/local/bin` 可写则用之）。
* `CNB_REPO` 或 `--repo OWNER/NAME`：自定义 GitHub 源（用于自托管/分叉）。

脚本会下载对应平台的归档、用 `.sha256` 校验、解压、`install -m 0755` 到目标目录。
**不会写入** shell rc 或 PATH；如目标目录不在 PATH，脚本会打印一行提示。

## Homebrew（macOS / Linux）

```bash
brew tap cnb-cool/tap
brew install cnb
```

> Tap 仓库由 maintainer 维护；模板见仓库内 `dist-templates/homebrew/`。

## Scoop（Windows）

```powershell
scoop bucket add cnb https://github.com/cnb-cool/scoop-bucket
scoop install cnb
```

## 从源码构建

需要 Rust 1.86+：

```bash
git clone https://cnb.cool/aodoo/tools/cnb-cli
cd cli
cargo install --path crates/cnb --locked
```

## 验证发布二进制（可选）

每个 release 资产都附带 cosign keyless 签名（`.sig` + `.pem`）。
详见 [Sigstore 验证发布二进制](./advanced/verify-binaries.md)。
