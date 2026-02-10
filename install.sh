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

release_headers=(-H "Accept: application/vnd.github+json")
asset_headers=(-H "Accept: application/octet-stream")
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  release_headers+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
  asset_headers+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

if [[ "$VERSION" == "latest" ]]; then
  release_api="https://api.github.com/repos/${OWNER_REPO}/releases/latest"
else
  release_api="https://api.github.com/repos/${OWNER_REPO}/releases/tags/${VERSION}"
fi

release_json="$(curl -fsSL "${release_headers[@]}" "$release_api")"

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required by installer" >&2
  exit 1
fi

tag="$(printf '%s' "$release_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
asset="rbazel-${tag}-${target}.tar.gz"
asset_api="$(printf '%s' "$release_json" | python3 -c 'import json,sys; j=json.load(sys.stdin); names=[sys.argv[1],sys.argv[2]];
for n in names:
  for a in j.get("assets",[]):
    if a.get("name")==n:
      print(a.get("url",""));
      raise SystemExit(0)
raise SystemExit(1)' "$asset" "rbazel-rs-${tag}-${target}.tar.gz")"

if [[ -z "$asset_api" ]]; then
  echo "ERROR: release asset not found: $asset or rbazel-rs-${tag}-${target}.tar.gz" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading release asset from $OWNER_REPO ..."
curl -fsSL "${asset_headers[@]}" "$asset_api" -o "$tmpdir/release-asset.tgz"

mkdir -p "$INSTALL_DIR"
tar -xzf "$tmpdir/release-asset.tgz" -C "$tmpdir"
if [[ -f "$tmpdir/rbazel" ]]; then
  install -m 0755 "$tmpdir/rbazel" "$INSTALL_DIR/rbazel"
elif [[ -f "$tmpdir/rbazel-rs" ]]; then
  install -m 0755 "$tmpdir/rbazel-rs" "$INSTALL_DIR/rbazel"
else
  echo "ERROR: extracted archive does not contain rbazel or rbazel-rs binary" >&2
  exit 1
fi

echo "Installed: $INSTALL_DIR/rbazel"
echo "If needed, add to PATH: export PATH=\"$INSTALL_DIR:\$PATH\""
