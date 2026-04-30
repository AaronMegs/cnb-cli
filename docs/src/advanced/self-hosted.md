# 自托管与企业部署

`cnb` 通过 `--hostname` / `CNB_HOST` / `default_host` 三层支持指向私有部署的 CNB 实例。
凡是涉及 URL 拼接的地方都走 `url_safe::resolve`，自动 percent-encode 路径段并防止 path traversal。

```bash
# 一次性
cnb --hostname cnb.example.internal repo list

# 持久化
cnb config set default_host cnb.example.internal

# 多 host 切换
cnb auth login --hostname cnb.example.internal
cnb auth status   # 列出所有已登录 host
```

## 自定义证书 / 私有 CA

若内网走自签证书，确保 OS 信任根证书 store 已包含；`cnb` 用 rustls 默认走系统信任。
如需走私有 CA bundle，可设置 `SSL_CERT_FILE` / `SSL_CERT_DIR`（rustls 会读取）。

## 代理

```bash
export HTTPS_PROXY=http://corp-proxy:8080
cnb api /user
```
