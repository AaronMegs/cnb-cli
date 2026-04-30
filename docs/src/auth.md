# 认证

`cnb` 用 Personal Access Token (PAT) 认证。Token 解析顺序固定为：

1. `CNB_TOKEN` 环境变量
2. 系统 keyring（macOS Keychain / Linux Secret Service / Windows Credential Store）
3. 文件 `~/.config/cnb/hosts.toml`

任意一级命中即停止；CI 通常用环境变量，本地开发推荐 keyring。

## 登录

```bash
cnb auth login                      # 交互式：粘贴 token
cnb auth login --with-token < t.txt # 从 stdin 读取（适合 CI 引导脚本）
cnb auth login --hostname cnb.cool  # 显式指定 host（默认就是 cnb.cool）
```

## 查询当前状态

```bash
cnb auth status
cnb auth token        # 仅打印 token，便于嵌入脚本
```

## 注销

```bash
cnb auth logout                # 当前 host
cnb auth logout --hostname X   # 指定 host
```

## 让 git 复用 cnb token

```bash
cnb auth setup-git
```

会在 `~/.gitconfig` 注册一个 credential helper，把 `cnb auth token` 的输出
作为 git push/pull 的密码，**不会** 在配置里写明文 token。

详见 `cnb auth --help`。
