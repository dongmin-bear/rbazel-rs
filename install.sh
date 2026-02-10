#!/usr/bin/env bash
set -euo pipefail

OWNER_REPO="${OWNER_REPO:-<OWNER>/rbazel-rs}"
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

mkdir -p "$INSTALL_DIR"

if command -v gh >/dev/null 2>&1; then
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  if [[ "$VERSION" == "latest" ]]; then
    tag="$(gh release view -R "$OWNER_REPO" --json tagName -q .tagName)"
  else
    tag="$VERSION"
  fi

  asset="rbazel-rs-${tag}-${target}.tar.gz"
  echo "Downloading $asset from $OWNER_REPO ..."
  gh release download -R "$OWNER_REPO" "$tag" -p "$asset" -D "$tmpdir"

  tar -xzf "${tmpdir}/${asset}" -C "$tmpdir"
  install -m 0755 "${tmpdir}/rbazel-rs" "${INSTALL_DIR}/rbazel-rs"
  echo "Installed: ${INSTALL_DIR}/rbazel-rs"
  exit 0
fi

echo "ERROR: gh not found. For private repos, install gh or provide a download method with auth." >&2
echo "Tip: https://cli.github.com/ then: gh auth login" >&2
exit 1