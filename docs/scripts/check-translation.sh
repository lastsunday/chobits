#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$script_dir/.." && pwd)"
content_dir="$root/content"
git_root="$(cd "$root" && git rev-parse --show-toplevel 2>/dev/null || echo "$root")"
root_rel="${root#$git_root/}"
stale=0

extract_frontmatter() {
  sed -n '/^+++/,/^+++/p' "$1"
}

get_hash_from_toml() {
  extract_frontmatter "$1" | grep '^source_hash' | sed 's/^source_hash = "\(.*\)"$/\1/' | head -1 || true
}

# Also check under [extra] section
get_hash_from_toml_extra() {
  extract_frontmatter "$1" | sed -n '/^\[extra\]/,/^+++/p' | grep '^source_hash' | sed 's/^source_hash = "\(.*\)"$/\1/' | head -1 || true
}

while IFS= read -r -d '' en_file; do
  en_rel="${en_file#$content_dir/}"
  cn_file="${en_file%.en.md}.md"

  # Check if source exists
  if [ ! -f "$cn_file" ]; then
    echo "⚠️  NO_SOURCE: $en_rel (missing $cn_file)"
    stale=1
    continue
  fi

  cn_rel="${cn_file#$git_root/}"
  current_hash=$(cd "$git_root" && git log -1 --format='%H' -- "$cn_rel" 2>/dev/null || echo "")
  stored_hash=$(get_hash_from_toml "$en_file" || true)
  if [ -z "$stored_hash" ]; then
    stored_hash=$(get_hash_from_toml_extra "$en_file" || true)
  fi

  if [ -z "$stored_hash" ]; then
    echo "⚠️  UNTRANSLATED: $en_rel"
    stale=1
  elif [ "$current_hash" != "$stored_hash" ]; then
    echo "⚠️  STALE: $en_rel (source changed: $stored_hash → $current_hash)"
    stale=1
  fi
done < <(find "$content_dir" -name '*.en.md' -print0)

exit $stale
