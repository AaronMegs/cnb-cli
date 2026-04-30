# Issue 与 PR

## Issue

```text
cnb issue list [REPO] --state {open|closed|all}
cnb issue view <number>
cnb issue create --title T [--body B]
cnb issue edit <number> [--title T --body B --state STATE]
cnb issue close / reopen <number>
cnb issue comment <number> --body B
cnb issue comment-edit <number> <comment-id> --body B
cnb issue assign <number> --add U1,U2 --remove U3
cnb issue label  <number> --add L1,L2 --remove L3
cnb issue activity   <number>
cnb issue properties <number>
```

## Pull Request（别名 `mr`）

```text
cnb pr list / view / create / edit / close / reopen / comment
cnb pr diff <number>
cnb pr commits <number>
cnb pr checkout <number>          # 拉出本地分支 pr/<number>
cnb pr assign / label
cnb pr merge <number> [--method {merge|squash|rebase}] [--delete-branch]
cnb pr review <number> --approve | --request-changes | --comment [--body B]
cnb pr checks <number>            # CI 状态
cnb pr batch 12 34 56 --json      # 批量取多个 PR 元数据
```

## Label

```text
cnb label list
cnb label create <name> [--color HEX --description TEXT]
cnb label edit <name>   [--new-name N --color HEX --description TEXT]
cnb label delete <name> --yes
```
