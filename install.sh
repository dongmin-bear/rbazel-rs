#!/usr/bin/env bash
set -euo pipefail

OWNER_REPO="${OWNER_REPO:-dongmin-bear/rbazel-rs}"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${1:-latest}"

uname_s="$(uname -s)"
uname_m="$(uname -m)"

if [[ "$uname_s" != "Linux" ]]; then
  echo "ERROR: installer currently supports Linux only (got: $uname_s)" >&2
  exit 1
fi

case "$uname_m" in
  x86_64) target="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
  *)
    echo "ERROR: unsupported arch: $uname_m" >&2
    exit 1
    ;;
esac

headers=(-H "Accept: application/vnd.github+json")
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  headers+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

if [[ "$VERSION" == "latest" ]]; then
  release_api="https://api.github.com/repos/${OWNER_REPO}/releases/latest"
else
  release_api="https://api.github.com/repos/${OWNER_REPO}/releases/tags/${VERSION}"
fi

release_json="$(curl -fsSL "${headers[@]}" "$release_api")"

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required by installer" >&2
  exit 1
fi

tag="$(printf '%s' "$release_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
asset="rbazel-rs-${tag}-${target}.tar.gz"
asset_api="$(printf '%s' "$release_json" | python3 -c 'import json,sys; j=json.load(sys.stdin); name=sys.argv[1];
for a in j.get("assets",[]):
  if a.get("name")==name:
    print(a.get("url",""));
    break
else:
  raise SystemExit(1)' "$asset")"

if [[ -z "$asset_api" ]]; then
  echo "ERROR: release asset not found: $asset" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading $asset from $OWNER_REPO ..."
curl -fsSL -H "Accept: application/octet-stream" "${headers[@]}" "$asset_api" -o "$tmpdir/$asset"

mkdir -p "$INSTALL_DIR"
tar -xzf "$tmpdir/$asset" -C "$tmpdir"
install -m 0755 "$tmpdir/rbazel-rs" "$INSTALL_DIR/rbazel-rs"

echo "Installed: $INSTALL_DIR/rbazel-rs"
echo "If needed, add to PATH: export PATH="$INSTALL_DIR:\$PATH""
