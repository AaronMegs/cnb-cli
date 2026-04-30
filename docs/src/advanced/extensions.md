# 扩展与别名

## 别名

参见 [配置文件 § cnb alias](../reference/config.md)。

## 直连 API：`cnb api`

任何尚未被顶层命令覆盖的端点都能通过 `cnb api` 兜底，类似 `gh api`：

```bash
cnb api /user
cnb api -X POST /repos/foo/bar/issues -f title='hi' -f body='from cli'
cnb api -X PATCH /repos/foo/bar/-/pulls/42 -F draft=false
cnb api /repos/foo/bar/issues --paginate --jq '.[].number'
```

支持：
* `-X / --method` HTTP 方法（默认 GET，有 `-f/-F` 时为 POST）
* `-f key=val` 字符串字段，`-F key=val` 会尝试解析为 number/bool/null
* `-H 'Header: Val'` 加 header
* `--jq` / `--template` / `--json` 与列表命令一致
* `--paginate` 自动翻页

## 第三方扩展（路线图 v1.x）

我们计划提供 `cnb extension {install,list,upgrade,remove}`，托管在 cnb.cool
组织下的 git 仓库即可被直接安装；当前版本暂未实现。
