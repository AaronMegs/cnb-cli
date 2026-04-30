# 快速上手：5 分钟之旅

假设已经 [安装](./install.md) 并 [登录](./auth.md)。

## 1. 看看自己

```bash
cnb api /user
```

## 2. 列出有权限的仓库

```bash
cnb repo list                    # 默认列首页 30 条
cnb repo list --limit 100        # 改分页
cnb repo list --json full_path,visibility,updated_at | jq '.[]'
```

## 3. 进入一个仓库目录后

```bash
cd ~/code/your-repo

cnb repo view                    # 自动从 git remote origin 推断 OWNER/REPO
cnb issue list --state open
cnb pr list --state open
```

## 4. 触发一次构建

```bash
cnb build run --branch main --pipeline ci
cnb build list                   # 看历史
cnb build logs <pipeline-id>     # 拉日志
cnb build watch <pipeline-id>    # 流式跟踪（带 spinner 与 ctrl-c）
```

## 5. 发布制品

```bash
cnb release create v1.0.0 --title "First stable" --notes-file CHANGELOG.md
cnb release upload v1.0.0 ./dist/*.tar.gz
cnb release download v1.0.0 myapp-darwin.tar.gz
```

更多命令参见 [命令参考](./reference/commands.md)。
