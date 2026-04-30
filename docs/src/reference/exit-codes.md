# 退出码

CLI 严格遵循以下退出码（与 `gh` CLI 高度对齐，便于脚本判断）：

| 码  | 含义                                                                |
| --- | ------------------------------------------------------------------- |
| 0   | 成功                                                                |
| 1   | 通用失败（未分类错误）                                              |
| 2   | 参数错误（clap 解析失败、不合法 enum、缺必填等）                    |
| 3   | I/O 错误（磁盘、stdin/stdout 中断）                                |
| 4   | 认证失败（token 缺失/失效，server 401）                             |
| 5   | 权限不足（403）                                                     |
| 6   | 资源不存在（404）                                                   |
| 7   | 冲突（409，例如重复创建标签）                                       |
| 8   | 用户取消（含 ctrl-c、二次确认拒绝、非 TTY 拒绝二次确认）            |
| 9   | 限流（429，附 Retry-After 提示）                                    |
| 10  | 上游 5xx                                                            |
| 11  | 网络/DNS 错误                                                       |
| 12  | TLS / 证书错误                                                      |
| 13  | API 协议异常（unexpected schema）                                   |
| 16  | CNB 业务错误（errcode≠0 且未映射到上面的码）                        |

脚本判断推荐：

```bash
if cnb pr view 42 >/dev/null 2>&1; then
  echo "exists"
else
  case $? in
    4) echo "needs login" ;;
    6) echo "no such PR" ;;
    *) echo "other failure" ;;
  esac
fi
```
