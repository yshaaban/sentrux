#!/usr/bin/env bash

prepare_typescript_benchmark_home() {
  local target_root="${1:?target root is required}"
  local source_home="${2:-$HOME}"
  local source_plugins="$source_home/.sentrux/plugins"
  local home_root="$target_root/home"
  local target_plugins="$home_root/.sentrux/plugins"
  local plugin_names=(
    typescript
    javascript
    json
    css
    scss
    html
    yaml
    toml
    bash
    markdown
  )

  mkdir -p "$target_plugins"

  for name in "${plugin_names[@]}"; do
    if [[ -d "$source_plugins/$name" ]]; then
      cp -R "$source_plugins/$name" "$target_plugins/$name"
    fi
  done

  if [[ ! -d "$target_plugins/typescript" ]]; then
    echo "Missing typescript plugin under $source_plugins" >&2
    return 1
  fi

  printf '%s\n' "$home_root"
}
