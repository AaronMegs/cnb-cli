#!/usr/bin/env bash
# cnb-pack.sh ─ 把交叉编译产物打包成发布 archive。
#
# 提取自 .cnb.yml 的 release pipeline，避免在 5 个 target stage 里
# 重复同一段 shell。逻辑与 .github/workflows/release.yml 的
# "Package archive" step 等价：
#
#   - Linux / macOS target → tar.gz
#   - Windows target       → zip（cnb runner 是 Linux，用 zip 命令；
#                              如果镜像没装 zip，apt 安一下）
#   - 同时生成 *.sha256 sidecar
#
# 用法：
#   bash scripts/cnb-pack.sh <target-triple> <ext>
#
# 参数：
#   target-triple ─ 例如 x86_64-unknown-linux-gnu / aarch64-apple-darwin
#   ext           ─ Windows 用 ".exe"，其它用 ""
#
# 环境变量：
#   BIN          ─ 二进制名（默认 cnb）
#   VERSION_TAG  ─ 版本标签（cnb 上等于 CNB_BRANCH，即触发 tag_push 的 tag 名）

set -euo pipefail

TARGET="${1:?usage: cnb-pack.sh <target> <ext>}"
EXT="${2:-}"
BIN="${BIN:-cnb}"
VERSION_TAG="${VERSION_TAG:-${CNB_BRANCH:-unknown}}"

NAME="${BIN}-${VERSION_TAG}-${TARGET}"
SRC_BIN="target/${TARGET}/release/${BIN}${EXT}"

# 确保 dist/ 与本次 archive 的工作子目录存在。
mkdir -p "dist/${NAME}"

# 产物本体。
cp "${SRC_BIN}" "dist/${NAME}/"

# 同 GitHub release.yml：把 README + DESIGN + LICENSE 一并打入 archive。
# || true 容错（仓库重命名 / LICENSE 文件名变化不阻塞打包）。
cp README.md DESIGN.md LICENSE* "dist/${NAME}/" 2>/dev/null || true

# man pages + shell completions（xtask gen-dist 已写入 dist/man /
# dist/completions，平台无关，所有 archive 都包含）。
mkdir -p "dist/${NAME}/man" "dist/${NAME}/completions"
cp -R dist/man/.        "dist/${NAME}/man/"        2>/dev/null || true
cp -R dist/completions/. "dist/${NAME}/completions/" 2>/dev/null || true

cd dist
if [[ "${TARGET}" == *windows* ]]; then
  # cnb runner 上 zip 命令默认未必装；保险起见 apt 安一下。
  command -v zip >/dev/null || (apt-get update && apt-get install -y --no-install-recommends zip)
  zip -r "${NAME}.zip" "${NAME}"
  shasum -a 256 "${NAME}.zip" > "${NAME}.zip.sha256"
  echo "✓ packed ${NAME}.zip"
else
  tar czf "${NAME}.tar.gz" "${NAME}"
  shasum -a 256 "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256"
  echo "✓ packed ${NAME}.tar.gz"
fi
