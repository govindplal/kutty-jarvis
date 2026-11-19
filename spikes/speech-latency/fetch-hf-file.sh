#!/usr/bin/env bash
# Download a Hugging Face file and verify it is complete, resuming if truncated.
# usage: fetch-hf-file.sh <repo> <file> <dest-dir>
set -euo pipefail
repo="$1"; file="$2"; dest="$3"
url="https://huggingface.co/${repo}/resolve/main/${file}"
mkdir -p "$dest"
expected=$(curl -sIL "$url" | tr -d '\r' | awk 'tolower($1)=="content-length:"{v=$2} END{print v}')
out="$dest/$(basename "$file")"
for _ in 1 2 3 4 5 6 7 8; do
  [ -f "$out" ] && [ "$(stat -c %s "$out")" = "$expected" ] && { echo "ok $out ($expected bytes)"; exit 0; }
  curl -sL -C - --retry 5 --retry-all-errors -o "$out" "$url" || true
done
echo "FAILED: $out is $(stat -c %s "$out" 2>/dev/null || echo 0) of $expected bytes" >&2
exit 1
