# Upstream issue drafts for the `cnb` SDK

Source-controlled drafts of the issues we plan to file against the
upstream `cnb` crate (a.k.a. `cnb-sdk` in our workspace
manifests; published by AaronMegs on crates.io as `cnb`).

These are **drafts** — meant to be copy-pasted into the upstream
issue tracker once Phase 2 of the cnb-cli typed-SDK migration is
complete.

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

## Tier B / Tier C

Tier B (DTO-completeness bundle, six items) and Tier C (housekeeping
meta-issue, eight items) are summarised in
[`../sdk-issues.md`](../sdk-issues.md) §75–105. They are not
drafted as standalone files yet — they are intentionally reported
as one consolidated issue each, so the maintainer can triage them
as a group.
