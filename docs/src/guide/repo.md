# 仓库与协作

## 命令清单

```text
cnb repo list                   列出可访问的仓库
cnb repo view [REPO]            查看仓库详情（默认从 git remote 推断）
cnb repo create OWNER/SLUG      新建仓库
cnb repo clone OWNER/REPO       克隆（透传给系统 git）
cnb repo fork [REPO]            派生
cnb repo edit [--name --description --default-branch]
cnb repo archive / unarchive    归档 / 解归档
cnb repo transfer --to OWNER    转移 owner
cnb repo set-visibility {public|internal|private}
cnb repo delete --yes           删除（需二次确认）
cnb repo pin / unpin            收藏
cnb repo contributors           列贡献者
```

## 自动 OWNER/REPO 推断

凡是接受 `[REPO]` 的命令，省略时会执行：

```bash
git remote get-url origin
```

支持 SSH/HTTPS/cnb.cool 三种 URL 形态；解析失败会清晰报错并提示
显式传 `--repo OWNER/REPO[/SUBGROUP]`。

## 破坏性命令

`delete` / `archive` / `set-visibility` 默认会提示二次确认。
传 `--yes` 跳过；非 TTY 环境（管道、CI）若没传 `--yes` 会直接拒绝（exit 8）。
