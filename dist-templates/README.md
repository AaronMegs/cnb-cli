# dist-templates/

Distribution-channel templates that are **not** consumed by `cargo` or by the
in-repo release CI. Each subdirectory targets one external delivery channel
where the manifest must live in a *separate* repository.

| Directory   | Channel               | Where the rendered file lives                                    |
| ----------- | --------------------- | ---------------------------------------------------------------- |
| `homebrew/` | macOS / Linux Homebrew| External tap repo (e.g. `cnb-cool/homebrew-tap`), `Formula/cnb.rb` |
| `scoop/`    | Windows Scoop         | External bucket repo (e.g. `cnb-cool/scoop-bucket`), `bucket/cnb.json` |

## Release-time workflow

After `release.yml` finishes and the `*.tar.gz.sha256` / `*.zip.sha256` files
are attached to the GitHub Release, run something like:

```bash
VER=0.4.0
DARWIN_AMD64=$(curl -fsSL "https://github.com/cnb-cool/cnb/releases/download/v${VER}/cnb-v${VER}-x86_64-apple-darwin.tar.gz.sha256" | awk '{print $1}')
DARWIN_ARM64=$(curl -fsSL "https://github.com/cnb-cool/cnb/releases/download/v${VER}/cnb-v${VER}-aarch64-apple-darwin.tar.gz.sha256"  | awk '{print $1}')
LINUX_AMD64=$( curl -fsSL "https://github.com/cnb-cool/cnb/releases/download/v${VER}/cnb-v${VER}-x86_64-unknown-linux-gnu.tar.gz.sha256" | awk '{print $1}')
LINUX_ARM64=$( curl -fsSL "https://github.com/cnb-cool/cnb/releases/download/v${VER}/cnb-v${VER}-aarch64-unknown-linux-gnu.tar.gz.sha256" | awk '{print $1}')
WIN_AMD64=$(   curl -fsSL "https://github.com/cnb-cool/cnb/releases/download/v${VER}/cnb-v${VER}-x86_64-pc-windows-msvc.zip.sha256"      | awk '{print $1}')

sed -e "s/%VERSION%/${VER}/g" \
    -e "s/%SHA256_DARWIN_AMD64%/${DARWIN_AMD64}/" \
    -e "s/%SHA256_DARWIN_ARM64%/${DARWIN_ARM64}/" \
    -e "s/%SHA256_LINUX_AMD64%/${LINUX_AMD64}/"   \
    -e "s/%SHA256_LINUX_ARM64%/${LINUX_ARM64}/"   \
    dist-templates/homebrew/cnb.rb.tmpl > /tmp/cnb.rb

sed -e "s/%VERSION%/${VER}/g" \
    -e "s/%SHA256_WIN_AMD64%/${WIN_AMD64}/" \
    dist-templates/scoop/cnb.json.tmpl > /tmp/cnb.json
```

Then commit `/tmp/cnb.rb` to the tap repo and `/tmp/cnb.json` to the bucket
repo. Both can be automated later via a small follow-up GH Action that pushes
to the sibling repos using a deploy key — out of scope for v0.4.

## Why templates and not generated artifacts?

Both Homebrew and Scoop expect the manifest to live in a dedicated repository
controlled by the maintainer. Generating these inline (e.g. via `cargo dist`'s
`installers.homebrew.tap` setting) requires a write token to the external
repo, which we deliberately keep out of band so first-time forkers don't
accidentally publish to a tap they don't own.
