# cnb — CNB CLI

[![ci](https://cnb.cool/cnb/cli/-/badge/ci)](https://cnb.cool/cnb/cli)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Official command-line tool for [CNB (CloudNative Build, cnb.cool)](https://cnb.cool), implemented in Rust and aligned with GitHub `gh` CLI conventions.

> **Status**: v0.4.0-alpha — 17 top-level commands; M4 done (registry / mission / org / browse / completion / config / alias) + M5.0 update check & release pipeline. See [Milestone status](#milestone-status).

## Quickstart

```bash
# 1. Login (paste your CNB Personal Access Token when prompted)
cnb auth login

# 2. Verify
cnb auth status

# 3. Call any OpenAPI endpoint directly
cnb api /user
cnb api /user/repos --jq '.[].name'

# 4. M2: high-level commands
cnb repo list                                  # repos accessible to your token
cnb repo view cnb/feedback                     # repo card
cnb issue list cnb/feedback --state=open
cnb issue create --title "bug" --body "..." cnb/feedback --attach screenshot.png
cnb issue close 42 cnb/feedback
cnb pr list cnb/feedback
cnb pr create --title "feat" --base main cnb/feedback   # head = current branch
cnb pr merge 7 cnb/feedback --method=squash --yes
cnb mr view 7 cnb/feedback                     # `mr` is an alias for `pr`
cnb label list cnb/feedback

# 5. M3: pipelines, workspaces, releases
cnb build run cnb/feedback --branch main       # trigger a build
cnb build status sn-123 cnb/feedback --watch   # poll until terminal state (ctrl-c safe)
cnb build logs pipeline-456 cnb/feedback --output runner.log
cnb workspace list                              # my dev environments
cnb workspace start cnb/feedback --branch main # opens webide URL in browser
cnb ws view --sn sn-x --web                    # alias `ws`; --web auto-opens
cnb release list cnb/feedback
cnb release create v1.2.0 --repo cnb/feedback --notes "Bug fixes"
cnb release upload r-1 dist/app.tar.gz --repo cnb/feedback --clobber
cnb release download v1.2.0 app.tar.gz --repo cnb/feedback --output ./downloads
cnb pr review 7 cnb/feedback --approve --body "LGTM"
cnb issue properties 42 cnb/feedback --set sev=high --set area=backend

# 6. M4: registry, mission, org, browse, completion, config, alias
cnb registry list cnb                          # registries under group `cnb`
cnb registry package list cnb --type npm
cnb registry tag view cnb --type npm --name foo --tag v1.0.0
cnb registry tag provenance cnb --type npm --name foo --tag v1.0.0
cnb mission view-list cnb/m1                   # views in a mission
cnb org list                                   # orgs you belong to
cnb org member add cnb alice --role write
cnb org follower alice                          # alice's followers
cnb browse                                      # open current repo in browser
cnb browse --issue 42                           # jump to an issue
cnb browse --no-browser                         # print URL only
cnb completion zsh > ~/.zsh/completions/_cnb
cnb config set core.git_protocol ssh
cnb config list
cnb alias set bugs 'issue list -l bug'
cnb alias list
cnb alias import < team-aliases.toml
cnb auth setup-git                              # configure git credential helper

# 7. M5.0: keep cnb up to date (opt-in only)
cnb update                                      # check GitHub Releases for new version
cnb update --check                              # quiet, just yes/no
```

## Installation

### One-liner (recommended once a release is published)

```bash
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh | bash

# Pin a specific version / install to a custom prefix:
curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh \
  | bash -s -- --version v0.4.0-alpha.1 --prefix ~/.local/bin
```

The installer auto-detects your platform, downloads the matching release archive
from GitHub, **verifies the SHA-256 checksum**, and installs the binary.
Honors `CNB_VERSION` / `CNB_PREFIX` / `CNB_REPO` env vars too.

### Build from source

```bash
git clone https://cnb.cool/cnb/cli
cd cli
cargo build --release
./target/release/cnb --help
```

Requires Rust 1.86+.

## Authentication

Tokens are resolved in the following order (see [DESIGN §5](DESIGN.md#5-认证子系统)):

1. **`CNB_TOKEN`** environment variable (CI / containers)
2. **System keyring** (macOS Keychain / Windows Credential Manager / Linux Secret Service)
3. **`~/.config/cnb/hosts.toml`** (fallback when keyring is unavailable)

```bash
# CI usage
export CNB_TOKEN=xxxxxxxxxxxxxxxxxxxxxxxx
cnb api /user

# Local usage
cnb auth login          # writes to keyring (or hosts.toml)
cnb auth token          # print current token (for piping)
cnb auth logout         # remove credentials
```

## `cnb api` — Generic REST passthrough

Aligned with `gh api`:

```bash
# GET
cnb api /user
cnb api /cnb/feedback/-/issues

# POST with fields
cnb api -X POST /cnb/feedback/-/issues -f title='Bug' -f body='Details...'

# Custom headers / show response headers / silent
cnb api /user -H 'X-Trace-Id: abc' -i
cnb api /user --silent

# Post-process with jq or template
cnb api /user --jq '.username'
cnb api /user --template '{username}: {email}'
```

## Documentation

- [DESIGN.md](DESIGN.md) — Architecture, command catalog, roadmap (single source of truth).
- `cnb --help`, `cnb <command> --help` — Built-in help.

## Milestone status

| Milestone | Status     | Scope                                                        |
| --------- | ---------- | ------------------------------------------------------------ |
| **M0**    | ✅ done    | Design freeze (DESIGN.md v0.1)                               |
| **M1**    | ✅ done    | Workspace skeleton + `cnb auth` + `cnb api`                  |
| **M2**    | ✅ done    | `cnb repo` / `cnb issue` / `cnb label` / `cnb pr` (39 subcommands; progenitor deferred to M2.x) |
| **M3**    | ✅ done    | `cnb build` / `cnb workspace` / `cnb release` + `cnb pr review/checks/batch` + `cnb issue activity/properties` (27 new subcommands) |
| **M4**    | ✅ done    | `cnb registry` / `cnb mission` / `cnb org` + `cnb repo collaborator/pin/contributors` + `cnb browse` / `cnb completion` / `cnb config` / `cnb alias` + `cnb auth setup-git` (35+ new subcommands) |
| **M5.0**  | ✅ done    | `cnb update`, `release.yml`, `scripts/install.sh`, CI hardening                |
| **M5.1**  | ✅ done    | man pages + 5-shell completions baked into release archives, cosign keyless signing, mdbook handbook scaffold, Homebrew/Scoop manifest templates |
| **SDK-1** | ✅ done    | Phase 1 of the cnb-api → typed SDK migration: depends on external crate `cnb = "0.2"`, pilots it via new `cnb search` (first consumer), keeps `cnb-api` facades in place for all other commands |
| **SDK-2** | in progress | Phase 2: port commands to SDK module-by-module. Done: `cnb repo view/list`, `cnb issue view/list`, `cnb pr view/list`. Next: `cnb label` full set, then writes, then `release/build/workspace` |
| **M5.2**  | partial    | apt / yum repos, Docker image — needs external infra (out of band) |
| **M6**    | partial    | sigstore signing ✅; mdbook deploy + external case study — out of band |

### M1 acceptance report

| # | Criterion                                                                  | Status |
|---|----------------------------------------------------------------------------|--------|
| 1 | `cargo build --workspace` succeeds                                         | ✅     |
| 2 | `cargo test --workspace` passes (72 unit + integration tests)              | ✅     |
| 3 | `cargo fmt --check` and `cargo clippy -D warnings` clean                   | ✅     |
| 4 | `cnb --help` exposes `auth` and `api` subcommands                          | ✅     |
| 5 | `CNB_TOKEN=… cnb api /user` returns user JSON against the real `cnb.cool`  | ✅     |
| 6 | `cnb auth login --with-token` writes to keyring, `auth status` reports it  | ✅     |
| 7 | When keyring is unavailable, falls back to `hosts.toml` with `0600` perms  | ✅     |
| 8 | `cargo xtask sync-openapi` writes `openapi/cnb-swagger-2.0.json`           | ✅     |
| 9 | No `crates/cnb-api/src/generated/` (deferred to M2 with progenitor)        | ✅     |
| 10| README quickstart documents the login → api `/user` flow                   | ✅     |

### M2 acceptance report

| # | Criterion                                                                                       | Status |
|---|--------------------------------------------------------------------------------------------------|--------|
| 1 | `cargo build --workspace --all-targets` succeeds                                                | ✅     |
| 2 | `cargo test --workspace -- --test-threads=1` passes (114 tests)                                 | ✅     |
| 3 | `cargo clippy --workspace --all-targets -- -D warnings` clean                                   | ✅     |
| 4 | `cnb --help` exposes `repo`/`issue`/`label`/`pr` (+`mr` alias)                                   | ✅     |
| 5 | `cnb repo` ships 11 subcommands; `cnb issue` ships 11; `cnb label` ships 4; `cnb pr` ships 13   | ✅     |
| 6 | All paths flow through `cnb-api::url_safe::resolve` (no string-concat URL anywhere)             | ✅     |
| 7 | `Context::resolve_repo` honors `--repo OWNER/REPO` and falls back to `git remote get-url origin`| ✅     |
| 8 | Destructive ops (`repo delete`, `pr merge`, `label delete`, `repo transfer`) require `--yes`/TTY | ✅     |
| 9 | Attachment uploader auto-detects file vs image, streams via `tokio-util::ReaderStream`          | ✅     |
| 10| `crates/cnb-api/src/generated/` still absent — progenitor integration deferred to M2.x          | ⏭     |

### M3 acceptance report

| #  | Criterion                                                                                       | Status |
|----|--------------------------------------------------------------------------------------------------|--------|
| 1  | `cargo build --workspace --all-targets` succeeds                                                | ✅     |
| 2  | `cargo test --workspace -- --test-threads=1` passes (151 tests, 0 failures)                     | ✅     |
| 3  | `cargo clippy --workspace --all-targets -- -D warnings` clean                                   | ✅     |
| 4  | `cnb --help` exposes 3 new subcommands: `build`/`workspace` (+`ws` alias)/`release`              | ✅     |
| 5  | `cnb build` ships 8 subcommands incl. `--watch` (tokio interval + indicatif spinner + ctrl-c)   | ✅     |
| 6  | `cnb workspace` ships 5 subcommands; `start` auto-opens webide URL via `open` crate              | ✅     |
| 7  | `cnb release` ships 9 subcommands incl. two-phase asset upload (URL → PUT → confirm)            | ✅     |
| 8  | `cnb pr review/checks/batch` and `cnb issue activity/properties` extend M2 commands              | ✅     |
| 9  | New service facades (`builds`/`workspaces`/`releases`) wiremock-tested at 13 unit cases         | ✅     |
| 10 | M3 integration suite covers TSV output, two-phase upload, alias resolution, `--watch` plumbing  | ✅     |

**Crate map (M1):** `cnb` (bin) · `cnb-cli` · `cnb-api` · `cnb-config` · `cnb-auth` · `cnb-git` · `cnb-tty` · `xtask`

(M2 / M3 / M4 add no new crates; `cnb-cli` accumulated `cnb-git` + `indicatif` + `open` + `clap_complete` + `toml` deps.)

## Development

```bash
# Build everything
cargo build --workspace

# Run tests (unit + integration)
cargo test --workspace

# Lint
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings

# Sync upstream OpenAPI spec (writes to openapi/)
cargo xtask sync-openapi
```

### Internal/testing env vars

| Variable           | Purpose                                                     |
| ------------------ | ----------------------------------------------------------- |
| `CNB_API_BASE`     | Override base URL (default `https://api.cnb.cool`). For wiremock-based integration tests only. |
| `CNB_CONFIG_DIR`   | Override config directory (CI / containers).                |
| `CNB_TOKEN`        | Bearer token (highest priority).                            |
| `CNB_HOST`         | Default host (default `cnb.cool`).                          |

## License

Dual-licensed under MIT or Apache-2.0.
