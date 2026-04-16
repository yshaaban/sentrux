#!/usr/bin/env bash
set -euo pipefail

TREE_SITTER_CLI_VERSION="${TREE_SITTER_CLI_VERSION:-0.25.10}"
TREE_SITTER_CLI_INSTALL_DIR="${TREE_SITTER_CLI_INSTALL_DIR:-$HOME/.local/tree-sitter-cli}"
TREE_SITTER_CLI_BIN_DIR="$TREE_SITTER_CLI_INSTALL_DIR/bin"
TREE_SITTER_CLI_BIN="$TREE_SITTER_CLI_BIN_DIR/tree-sitter"

function current_version() {
  "$TREE_SITTER_CLI_BIN" --version 2>/dev/null | awk 'NR == 1 { print $2 }'
}

function add_bin_dir_to_path() {
  if [[ -n "${GITHUB_PATH:-}" ]]; then
    printf '%s\n' "$TREE_SITTER_CLI_BIN_DIR" >>"$GITHUB_PATH"
    return
  fi

  printf 'tree-sitter CLI installed at %s\n' "$TREE_SITTER_CLI_BIN"
}

function install_cli() {
  cargo install \
    --locked \
    --force \
    --root "$TREE_SITTER_CLI_INSTALL_DIR" \
    --version "$TREE_SITTER_CLI_VERSION" \
    tree-sitter-cli
}

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to install tree-sitter-cli" >&2
  exit 1
fi

if [[ -x "$TREE_SITTER_CLI_BIN" ]]; then
  installed_version="$(current_version || true)"
  if [[ "$installed_version" == "$TREE_SITTER_CLI_VERSION" ]]; then
    add_bin_dir_to_path
    echo "Using cached tree-sitter CLI $installed_version"
    exit 0
  fi
fi

mkdir -p "$TREE_SITTER_CLI_BIN_DIR"
install_cli

installed_version="$(current_version || true)"
if [[ "$installed_version" != "$TREE_SITTER_CLI_VERSION" ]]; then
  echo "Expected tree-sitter CLI $TREE_SITTER_CLI_VERSION but found ${installed_version:-missing}" >&2
  exit 1
fi

add_bin_dir_to_path
echo "Installed tree-sitter CLI $installed_version"
