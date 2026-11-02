#!/bin/sh
# Per-platform release signing (T4337). Usage: sign.sh <file>...
# Writes <file>.sha256 for every file and, when MINISIGN_SECRET_KEY is set, <file>.minisig. Installs minisign 0.12
# pinned by SHA-256 (archives verified against the minisign author's key when pinned, 2026-11-02).
set -eu
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi; }
for f in "$@"; do
  (cd "$(dirname "$f")" && sha256 "$(basename "$f")") > "$f.sha256"
done
if [ -z "${MINISIGN_SECRET_KEY:-}" ]; then
  echo "::warning::MINISIGN_SECRET_KEY is not set: assets are unsigned (auto-update skips them)"
  exit 0
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fetch() {
  curl -fsSL -o "$work/$1" "https://github.com/jedisct1/minisign/releases/download/0.12/$1"
  [ "$(sha256 "$work/$1" | cut -d' ' -f1)" = "$2" ] || { echo "minisign archive hash mismatch: $1" >&2; exit 1; }
}
case "$(uname -s)" in
  Linux) command -v minisign >/dev/null 2>&1 || { sudo apt-get update -qq && sudo apt-get install -y -qq minisign; }
    tool=minisign ;;
  Darwin) fetch minisign-0.12-macos.zip 89000b19535765f9cffc65a65d64a820f433ef6db8020667f7570e06bf6aac63
    (cd "$work" && unzip -q minisign-0.12-macos.zip) && tool="$work/minisign" && chmod +x "$tool" ;;
  MINGW*|MSYS*|CYGWIN*) fetch minisign-0.12-win64.zip 37b600344e20c19314b2e82813db2bfdcc408b77b876f7727889dbd46d539479
    (cd "$work" && 7z x -y minisign-0.12-win64.zip >/dev/null)
    arch=x86_64; [ "${PROCESSOR_ARCHITECTURE:-}" = ARM64 ] && arch=aarch64
    tool="$work/minisign-win64/$arch/minisign.exe" ;;
  *) echo "unsupported OS $(uname -s)" >&2; exit 2 ;;
esac
umask 077
# minisign needs the comment line; accept a secret holding only the key line too.
case "$MINISIGN_SECRET_KEY" in
  "untrusted comment:"*) printf '%s\n' "$MINISIGN_SECRET_KEY" > "$work/key" ;;
  *) printf 'untrusted comment: minisign secret key\n%s\n' "$MINISIGN_SECRET_KEY" > "$work/key" ;;
esac
for f in "$@"; do
  "$tool" -S -s "$work/key" -m "$f" -t "tsfp-launcher ${GITHUB_REF_NAME:-local}" </dev/null
done
echo "signed $# file(s)"
