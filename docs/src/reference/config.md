# 配置文件

两份独立文件，schema v1：

| 文件                              | 内容                                            |
| --------------------------------- | ----------------------------------------------- |
| `~/.config/cnb/config.toml`       | 用户偏好（默认 host、别名、`prompt`、`pager` 等） |
| `~/.config/cnb/hosts.toml`        | 每 host 一段：`token`、`user`、`git_protocol` 等  |

> macOS：`~/Library/Application Support/cnb/`；
> Windows：`%APPDATA%\cnb\`。

写入采用临时文件 + `rename` + 文件锁（`fs2`），保证不会半写损坏。

## `cnb config`

```text
cnb config get <key>
cnb config set <key> <value>
cnb config list
```

支持的 key：`prompt`, `pager`, `git_protocol`, `editor`, `default_host`, ...
（具体见 `cnb config --help`）。

## `cnb alias`

```text
cnb alias set co 'pr checkout'
cnb alias list
cnb alias delete co
cnb alias import < aliases.yaml
```

别名是纯字符串前缀替换，递归一次。
