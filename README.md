# rbazel-rs

`rbazel` runs Bazel on a remote server using your local git `HEAD`, then pulls artifacts back to your machine.

Artifacts are extracted to `./_rbazel_artifacts/<branch>/<timestamp>/`.

## Quick Start

```bash
curl -fsSL https://raw.githubusercontent.com/dongmin-bear/rbazel-rs/main/install.sh | bash
rbazel build //...
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/dongmin-bear/rbazel-rs/main/install.sh | bash -s -- v0.1.0
```

Private repo use:

```bash
curl -fsSL https://raw.githubusercontent.com/dongmin-bear/rbazel-rs/main/install.sh | GITHUB_TOKEN=ghp_xxx bash
```

## Installation

From source:

```bash
cargo build --release
install -m 0755 target/release/rbazel ~/.local/bin/rbazel
```

Make sure `~/.local/bin` is in your `PATH`.

## Configuration

Config file lookup order:

1. `./rbazel_config.toml`
2. `~/.config/rbazel/config.toml`

Supported keys under `[rbazel]`:

- `server_host`: SSH destination, e.g. `user@10.0.0.10`
- `server_repo_dir`: absolute path to remote repo
- `remote_cache_base`: remote Bazel cache/output base path
- `local_pull_base`: local artifact base path
- `remote_resource_path` (optional): if set, used directly; if not set, defaults to `${remote_cache_base}/pennybot`

Fallback environment variables:

- `SERVER_HOST`
- `SERVER_REPO_DIR`
- `REMOTE_CACHE_BASE`
- `LOCAL_PULL_BASE`

If config file values are present, they override env/default values.

Use `rbazel_config.example.toml` as your template.

## Usage

```bash
rbazel [bazel] <subcommand> [bazel-args...]
```

Examples:

```bash
rbazel version
rbazel build //...
rbazel test //foo:bar
rbazel build --config=aarch64_musl //system/...:target
rbazel bazel build //...
```

## How It Works

- Verifies local dependencies: `ssh`, `rsync`, `git`, `tar`
- Requires a clean local git tree
- Resolves local `HEAD` and branch
- SSHes to remote server, syncs and checks out the same commit
- Runs Bazel remotely and packages artifacts
- Pulls and extracts artifacts locally

## Requirements

Local:

- `ssh`, `rsync`, `git`, `tar`

Remote:

- `bash`, `git`, `bazel`, `tar`
- `find`, `sed`, `grep`

## Troubleshooting

- Dirty working tree: commit/stash first
- Commit not found on server: verify remote repo path and fetch state
- SSH failure: verify `server_host` and SSH auth
