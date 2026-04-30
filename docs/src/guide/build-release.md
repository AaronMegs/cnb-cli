# 构建与发布

## Build（流水线）

```text
cnb build run --branch B --pipeline P [--env K=V]...
cnb build list / status <sn> / view <sn>
cnb build logs <pipeline-id>           # 一次性拉日志
cnb build watch <pipeline-id>          # 流式跟踪，spinner+ctrl-c
cnb build cancel <sn>
cnb build delete-logs <sn>
cnb build crontab-sync <branch>
```

## Workspace（云原生开发环境，别名 `ws`）

```text
cnb workspace list / start / view --sn S / stop / delete
```

`start` 返回的 URL 可以 `cnb browse` 直接打开。

## Release

```text
cnb release list  --limit N --page P
cnb release view  TAG | --id ID | --latest
cnb release create TAG [--title --notes --notes-file - --draft --prerelease --target REF --asset PATH]
cnb release edit   ID   [--title --notes --notes-file --draft --prerelease]
cnb release delete ID --yes
cnb release upload ID FILE...   [--clobber --ttl DAYS]    # 两段式：pre-signed PUT + verify POST
cnb release download TAG FILENAME [--output DIR]
cnb release asset-view   ID ASSET_ID
cnb release asset-delete ID ASSET_ID --yes
```

> 所有 release 子命令的 `--repo OWNER/REPO[/SUBGROUP]` 是 **flag** 而非
> 位置参数（避免 `release view cnb/feedback` 的歧义；与 `gh release` 一致）。
