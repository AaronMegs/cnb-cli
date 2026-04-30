# cnb CLI

`cnb` 是 [CNB（CloudNative Build, cnb.cool）](https://cnb.cool) 平台的官方命令行工具，
用 Rust 实现，命令模型与 GitHub `gh` CLI 完全对齐——熟悉 `gh` 的开发者可以零成本切换。

## 目标

* **开发者终端入口**：在 macOS / Linux / Windows 终端完成日常 CNB 平台操作，无需打开浏览器。
* **覆盖 14 大命令组**：`auth / repo / issue / pr / release / build / workspace / registry / mission / org / api / browse / completion / config(+alias)`。
* **可脚本化**：所有列表/详情命令支持 `--json [fields]` / `--jq <expr>` / `--template <tpl>`，非 TTY 自动降级为无色 TSV。
* **三级 Token 解析**：`CNB_TOKEN` 环境变量 > 系统 keyring > 文件 `~/.config/cnb/hosts.toml`，CI 与本地两端友好。

## 快速试用

```bash
# 安装（一行）
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh | bash

# 登录
cnb auth login --hostname cnb.cool

# 查看自己
cnb api /user
```

继续阅读 [快速上手](./quickstart.md)。

## 文档版本

本手册对应 cnb CLI **v0.4.x** 系列。源代码与设计文档：
<https://cnb.cool/cnb/cli>。
