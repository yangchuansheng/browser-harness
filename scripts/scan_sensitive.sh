#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

if ! command -v rg >/dev/null 2>&1; then
  echo "scan_sensitive.sh requires ripgrep (rg)" >&2
  exit 2
fi

mapfile -d '' files < <(git ls-files -z --cached --others --exclude-standard)

if ((${#files[@]} == 0)); then
  echo "No tracked or unignored files to scan."
  exit 0
fi

fail=0

scan() {
  local label=$1
  local pattern=$2
  local tmp
  tmp=$(mktemp)
  if rg -nI -H --pcre2 --color=never -e "$pattern" "${files[@]}" >"$tmp"; then
    echo "Sensitive content detected: $label" >&2
    cat "$tmp" >&2
    fail=1
  fi
  rm -f "$tmp"
}

scan "Browser Use API key" 'BROWSER_USE_API_KEY\s*=\s*bu_[A-Za-z0-9_-]{20,}'
scan "OpenAI API key" 'OPENAI_API_KEY\s*=\s*sk-[A-Za-z0-9_-]{20,}'
scan "Anthropic API key" 'ANTHROPIC_API_KEY\s*=\s*sk-ant-[A-Za-z0-9_-]{20,}'
scan "AWS access key" '\bAKIA[0-9A-Z]{16}\b'
scan "Google API key" '\bAIza[0-9A-Za-z_-]{35}\b'
scan "GitHub token" '\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b'
scan "Slack token" '\bxox[baprs]-[A-Za-z0-9-]{10,}\b'
scan "Private key block" '-----BEGIN [A-Z ]*PRIVATE KEY-----'
scan "Local Linux home path" '/home/[A-Za-z0-9._-]+/'
scan "Local macOS home path" '/Users/[A-Za-z0-9._-]+/'
scan "Local Windows user path" 'C:\\Users\\[^\\[:space:]]+'
scan "Pinned local Chrome websocket" 'ws://127\.0\.0\.1:9222/'

if ((fail)); then
  exit 1
fi

echo "No obvious secrets or local path leaks found in tracked/unignored files."
