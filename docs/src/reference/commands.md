# 命令参考

完整命令列表参见随发布二进制分发的 man pages：

```bash
man cnb           # 顶层概览
man cnb-repo      # cnb repo 子命令
man cnb-pr-merge  # 任意叶子命令
```

man pages 由 `cargo xtask gen-man` 从 clap 定义生成；release CI 会把
`man/` 目录打进每个平台的 tar.gz / zip。Homebrew formula 会自动 `man1.install`，
其他渠道（手动安装 / install.sh）需要把 `man/` 复制到 `$prefix/share/man/man1/`。

也可以在终端实时查看：

```bash
cnb help
cnb repo --help
cnb release upload --help
```

## 14 大命令组速查

| 命令组       | 一句话                                  |
| ------------ | ---------------------------------------- |
| `auth`       | 三级 token 解析 + git credential helper |
| `repo`       | 仓库 CRUD、归档、转移、可见性、协作者    |
| `issue`      | issue CRUD + 标签 + 指派 + 评论 + 活动   |
| `pr` (`mr`)  | PR CRUD + diff/commits/checkout + review/checks/merge |
| `label`      | 标签 CRUD                                |
| `build`      | 流水线触发、状态、日志、watch、定时同步  |
| `workspace`  | 云原生开发环境生命周期                   |
| `release`    | release CRUD + 两段式 asset 上传/下载    |
| `registry`   | 11 种制品库的包/标签/规则/钩子           |
| `mission`    | 任务集                                   |
| `org`        | 组织、群组、成员                         |
| `api`        | 直连 REST，类似 `gh api`                 |
| `browse`     | 浏览器跳转                               |
| `completion` | 5-shell 补全脚本输出                     |
| `config`     | 用户配置读写                             |
| `alias`      | 用户别名                                 |
| `update`     | 自更新（参见 [安装](../install.md)）     |
