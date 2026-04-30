# 脚本化输出

所有"列表 / 详情"型命令都支持三种结构化输出，**互不冲突，按以下优先级**：

1. `--template '<tinytemplate>'` — 若提供则只走模板。
2. `--jq '<expr>'` — 否则若提供 jq 表达式，对原始 JSON 应用过滤。
3. `--json` — 否则若 `--json`，输出完整 JSON。
4. 默认：人类可读表格 / 详情卡片。

## 例子

```bash
# 全量 JSON
cnb repo list --json

# jq 过滤
cnb repo list --jq '.[] | {p: .full_path, vis: .visibility}'

# 模板（tinytemplate，{ } 包语法）
cnb pr list --template '{{ for p in this }}{{ p.number }} {{ p.title }}\n{{ endfor }}'
```

## 终端检测

* TTY → 彩色、对齐表格、Markdown 详情；
* 管道 / 重定向 / `NO_COLOR=1` → 无色、TSV、纯文本；
* `--json/--jq/--template` 总是无色，方便嵌入 shell 流水线。

## 退出码

* `0` 成功
* `1` 通用错误
* `2` 参数错误（clap）
* `4` 认证失败
* `8` 用户取消（含 "non-TTY 拒绝二次确认"）
* `>=10` API/网络错误（详见 [退出码](../reference/exit-codes.md)）
