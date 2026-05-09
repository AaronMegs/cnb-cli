# Upstream issue drafts for the `cnb` SDK

<!-- markdownlint-disable MD060 -->
<!-- MD060: aligned-table widening hurts the at-a-glance navigation
     these tables are designed for. -->

Source-controlled drafts of the issues we plan to file against the
upstream `cnb` crate (a.k.a. `cnb-sdk` in our workspace
manifests; published by AaronMegs on crates.io as `cnb`).

These are **drafts** — meant to be copy-pasted into the upstream
issue tracker once Phase 2 of the cnb-cli typed-SDK migration is
complete.

## 中文整合版（推荐给上游使用）

[`SDK-反馈汇总.md`](./SDK-反馈汇总.md) 把全部 19 个问题、A/B/C 三级
分组、提单顺序、复现锚点和待外部决定事项整合到一份单文件，使用中文
撰写，便于直接转发给上游维护者，也便于后续自查。下面的英文 minimal
repro 文件作为它的附件保留，提供独立可贴的代码片段。

## Tier A — file as standalone issues

Each of the five files below is a self-contained reproduction. They
are the highest-leverage cases: every consumer has to reinvent the
same workaround, *and* the workaround loses a real SDK feature
(typed access, connection-pool reuse, auth forwarding).

| File                           | Id        | One-line pitch                                                                                  |
|--------------------------------|-----------|-------------------------------------------------------------------------------------------------|
| [`SDK-I03.md`](./SDK-I03.md)   | SDK-I03   | `Visibility` alias does not accept integer-form server responses                                |
| [`SDK-I07.md`](./SDK-I07.md)   | SDK-I07   | Issue-vs-pull number typing disagrees across methods (`i64` vs `String`)                        |
| [`SDK-I09.md`](./SDK-I09.md)   | SDK-I09   | `Pull.{head,base}` typed as `Option<Value>` — every UI reinvents `read_branch`                  |
| [`SDK-I14.md`](./SDK-I14.md)   | SDK-I14   | No non-JSON transport — bytes endpoints need a side-car `reqwest::Client`                       |
| [`SDK-I15.md`](./SDK-I15.md)   | SDK-I15   | `list_package_tags` returns single-object git-`Tag`, not `Vec<RegistryPackageTag>`              |

Each draft links back to the canonical entry in
[`../sdk-issues.md`](../sdk-issues.md) so the longer-form context
(Severity, Workaround, Desired fix) stays in one place.

## Anchors

All drafts cite SDK version `0.2.x` (workspace dependency
`cnb-sdk = { package = "cnb", version = "0.2", default-features =
false, features = ["rustls-tls", "retry", "all-resources"] }`) and
reference the `cnb` (CLI) commit they were observed in. The five
drafts here cover all the workarounds present in the tree at
`b785d35` (`sdk(step-2.11)`).

## Tier B / Tier C — file as one consolidated issue each

Tier B and Tier C are intentionally reported as **one issue
each**, not item-by-item, so the maintainer can triage related
fixes as a group.

| File                           | Tier   | One-line pitch                                                                                  |
|--------------------------------|--------|-------------------------------------------------------------------------------------------------|
| [`Tier-B.md`](./Tier-B.md)     | Tier B | DTO completeness & method-signature consistency (6 sub-items: SDK-I01 / I02 / I08 / I11 / I13 / I19) |
| [`Tier-C.md`](./Tier-C.md)     | Tier C | Polish & conventions meta-issue (8 sub-items: SDK-I04 / I05 / I06 / I10 / I12 / I16 / I17 / I18)     |

Both files cross-link back to [`../sdk-issues.md`](../sdk-issues.md)
for the longer-form context.
