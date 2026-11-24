#!/bin/sh
set -eu

REPO="${SENN_REPO:-0PandaDEV/senn}"
VERSION="${SENN_VERSION:-latest}"

fail() {
  echo "error: $*" >&2
  exit 1
}

[ "$(uname -s)" = "Linux" ] || fail "linux only"
[ "$(id -u)" -eq 0 ] || fail "run as root (use sudo)"
command -v systemctl >/dev/null 2>&1 || fail "systemd required"
command -v docker >/dev/null 2>&1 || echo "warning: docker not found on this host" >&2

case "$(uname -m)" in
  x86_64 | amd64) arch=amd64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) fail "unsupported architecture $(uname -m)" ;;
esac

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -qO "$2" "$1"; }
else
  fail "curl or wget required"
fi

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/$REPO/releases/latest/download"
else
  base="https://github.com/$REPO/releases/download/$VERSION"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

name="senn-agent-linux-$arch"
echo "downloading $name ($VERSION)"
fetch "$base/$name" "$tmp/$name" || fail "download failed: $base/$name"
fetch "$base/checksums.txt" "$tmp/checksums.txt" || fail "download failed: $base/checksums.txt"

expected="$(awk -v n="$name" '$2 == n { print $1 }' "$tmp/checksums.txt")"
[ -n "$expected" ] || fail "no checksum for $name"
actual="$(sha256sum "$tmp/$name" | awk '{ print $1 }')"
[ "$expected" = "$actual" ] || fail "checksum mismatch"

chmod +x "$tmp/$name"

if [ "$#" -eq 0 ]; then
  install -m 0755 "$tmp/$name" /usr/local/bin/senn-agent
  echo "installed /usr/local/bin/senn-agent"
  echo "next: sudo senn-agent install --url <panel url> --token <token> --stacks-dir /opt/stacks"
  exit 0
fi

"$tmp/$name" install "$@"
