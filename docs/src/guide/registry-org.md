# 制品仓库与组织

## Registry（制品库）

`--type` 必须为下列 11 种之一（CLI 会做白名单校验）：
`docker | helm | maven | npm | pypi | rubygems | composer | nuget | golang | conan | generic`。

```text
cnb registry list   --type T  [--limit N --page P]
cnb registry view   --type T --name NAME
cnb registry stats  --type T
cnb registry packages list   --type T --name NAME
cnb registry packages view   --type T --name NAME --version V
cnb registry packages delete --type T --name NAME --version V --yes
cnb registry tags    list/add/remove
cnb registry rules   list/set
cnb registry hooks   list/test
```

## Mission（任务集）

```text
cnb mission list / view / create / edit / delete / run
```

## Org（组织 / 群组）

```text
cnb org list / view / members
cnb org member add/remove/role
```

## Repo 周边

```text
cnb repo collaborator list/add/remove   # （走 cnb api 兜底，端点 swagger 未暴露）
cnb repo activity                        # （同上）
cnb repo contributors
cnb repo pin / unpin
```
