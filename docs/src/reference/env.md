# 环境变量

| 变量                       | 作用                                                |
| -------------------------- | --------------------------------------------------- |
| `CNB_TOKEN`                | 优先级最高的 token 源（CI 必备）                    |
| `CNB_HOST`                 | 默认 host（等价于 `--hostname`）                   |
| `CNB_KEYRING_BACKEND=none` | 关闭 keyring，强制走文件回退（远程容器/WSL 友好） |
| `CNB_CONFIG_DIR`           | 覆盖配置目录（默认按 OS 标准位置）                  |
| `CNB_NO_UPDATE_CHECK=1`    | 关闭 `cnb update` 后台版本检查                      |
| `NO_COLOR=1`               | 强制无色输出（与 stdout TTY 检测正交）              |
| `RUST_LOG`                 | 调试日志级别（`debug` / `trace`）                   |
| `HTTPS_PROXY` / `HTTP_PROXY` | 走代理（reqwest 默认尊重）                       |
