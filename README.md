# rbazel-rs

`rbazel-rs` is a Rust implementation of the `rbazel` workflow: run Bazel on a remote machine against the *exact* local git `HEAD`, then pull the resulting build artifacts back to your local machine.

It is config-driven (remote host + remote repo path + cache paths) and intended for setups where remote hardware/OS is required (e.g., dedicated build servers).

## What it does

- Verifies required local commands exist: `ssh`, `rsync`, `git`, `tar`
- Requires a clean local git working tree (no staged/unstaged changes)
- Computes your current `HEAD` and branch name
- SSHes to the configured remote server
- On the remote server:
  - fetches updates
  - checks out the local `HEAD` commit (detached)
  - runs `bazel <subcommand> [opts] [targets]` with a remote output/cache root
  - collects output files and creates a tarball
- `rsync`s the tarball back locally and extracts it
- Prints the output directory

Artifacts are extracted to:

`./_rbazel_artifacts/<branch>/<timestamp>/`

## Installation

### Option A: Install from GitHub Releases (recommended)

This repo includes `install.sh` which downloads a release asset with `curl` and installs `rbazel-rs` into `~/.local/bin` (configurable).

```bash
curl -fsSL https://raw.githubusercontent.com/dmkim/rbazel-rs/main/install.sh | bash
curl -fsSL https://raw.githubusercontent.com/dmkim/rbazel-rs/main/install.sh | bash -s -- v0.1.0
```

Private repo note: set `GITHUB_TOKEN` before running the script.

```bash
curl -fsSL https://raw.githubusercontent.com/dmkim/rbazel-rs/main/install.sh | GITHUB_TOKEN=ghp_xxx bash
```

### Option B: Build locally

```bash
cargo build --release
install -m 0755 target/release/rbazel-rs ~/.local/bin/rbazel-rs
```

Ensure `~/.local/bin` is on your `PATH`.

## Configuration

`rbazel-rs` loads config from TOML. It looks for a config file in this order:

1. `./rbazel_config.toml`
2. `~/.config/rbazel/config.toml`

Supported keys live under the `[rbazel]` table:

- `server_host` (string) - SSH destination, e.g. `user@10.0.0.10`
- `server_repo_dir` (string) - absolute path to the repo on the remote host
- `remote_cache_base` (string) - remote base directory for Bazel output/cache
- `local_pull_base` (string) - local base directory for extracted artifacts
- `remote_resource_path` (string, optional) - if set, used directly; otherwise defaults to `${remote_cache_base}/pennybot`

### Precedence (important)

Defaults are derived from environment variables, then overridden by the config file if present.

Environment variables (used as defaults):

- `SERVER_HOST`
- `SERVER_REPO_DIR`
- `REMOTE_CACHE_BASE`
- `LOCAL_PULL_BASE`

If a config file is found, any keys set under `[rbazel]` override those defaults.

### Example config

See `rbazel_config.example.toml` for a template. Recommended setup:

- Keep a machine-local config at `~/.config/rbazel/config.toml`
- Optionally place a repo-local `rbazel_config.toml` for per-repo overrides
- Do not commit `rbazel_config.toml` (this repo ignores it by default)

## Usage

`rbazel-rs` syntax mirrors Bazel:

```bash
rbazel-rs [bazel] <subcommand> [bazel-args...]
```

Examples:

```bash
rbazel-rs version
rbazel-rs build //...
rbazel-rs test //foo:bar
rbazel-rs build --config=aarch64_musl //system/...:target

# "bazel" prefix is accepted and ignored:
rbazel-rs bazel build //...
```

Notes on argument parsing:

- The first argument is treated as the Bazel subcommand (`build`, `test`, `run`, `query`, etc.)
- Arguments that look like targets are treated as targets:
  - start with `//`
  - start with `:`
  - start with `@` and contain `//`
- Everything else is passed as options

## Requirements

Local machine:

- `ssh`, `rsync`, `git`, `tar`

Remote machine:

- `bash`, `git`, `bazel`, `tar`
- common Unix tools (`find`, `sed`, `grep`) used by the remote script

## Troubleshooting

- “dirty working tree”: commit/stash changes before running
- “commit not found on server”: ensure the remote repo has access to the commit (fetch works, correct remote repo dir, correct permissions)
- SSH failures: verify `server_host` and your SSH keys/agent
