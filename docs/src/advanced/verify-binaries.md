# 用 Sigstore 验证发布二进制

从 v0.4.0 起，每个 release 资产都有 [cosign keyless](https://docs.sigstore.dev/cosign/signing/overview/)
签名（GitHub Actions OIDC + Sigstore Fulcio，无需对称密钥）。

每个归档（`*.tar.gz` / `*.zip`）旁边附带：

* `*.sha256` — 完整性校验（install.sh 自动用）
* `*.sig`    — cosign 签名
* `*.pem`    — Fulcio 临时证书

## 验证流程

```bash
# 装 cosign
brew install cosign            # 或 https://docs.sigstore.dev/cosign/installation/

ARCHIVE=cnb-v0.4.0-aarch64-apple-darwin.tar.gz
BASE=https://github.com/cnb-cool/cnb/releases/download/v0.4.0

curl -fLO $BASE/$ARCHIVE
curl -fLO $BASE/$ARCHIVE.sha256
curl -fLO $BASE/$ARCHIVE.sig
curl -fLO $BASE/$ARCHIVE.pem

# 1. SHA-256
shasum -a 256 -c $ARCHIVE.sha256

# 2. cosign keyless verify
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --certificate $ARCHIVE.pem \
  --signature   $ARCHIVE.sig \
  --certificate-identity-regexp 'https://github.com/cnb-cool/cnb/.+' \
  --certificate-oidc-issuer     https://token.actions.githubusercontent.com \
  $ARCHIVE
```

通过 = 该归档由 `cnb-cool/cnb` 仓库的 release.yml workflow 在 GitHub OIDC 下签出。
