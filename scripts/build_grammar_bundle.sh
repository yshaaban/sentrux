#!/usr/bin/env bash
set -euo pipefail

function usage() {
  cat <<'EOF'
Usage: build_grammar_bundle.sh --platform <platform> --output <tar.gz path>

Supported platforms:
  darwin-arm64
  linux-x86_64
  linux-aarch64
EOF
}

function parse_args() {
  PLATFORM=""
  OUTPUT=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --platform)
        PLATFORM="${2:-}"
        shift 2
        ;;
      --output)
        OUTPUT="${2:-}"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
  done

  if [[ -z "$PLATFORM" || -z "$OUTPUT" ]]; then
    usage >&2
    exit 1
  fi
}

function configure_platform() {
  case "$PLATFORM" in
    darwin-arm64)
      GRAMMAR_EXT="dylib"
      CC_BIN="cc"
      CXX_BIN="c++"
      CC_FLAGS=(-arch arm64)
      LINK_FLAGS=(-dynamiclib -undefined dynamic_lookup)
      ;;
    linux-x86_64)
      GRAMMAR_EXT="so"
      CC_BIN="cc"
      CXX_BIN="c++"
      CC_FLAGS=()
      LINK_FLAGS=(-shared)
      ;;
    linux-aarch64)
      GRAMMAR_EXT="so"
      if [[ "$(uname -m)" == "aarch64" || "$(uname -m)" == "arm64" ]]; then
        CC_BIN="cc"
        CXX_BIN="c++"
      else
        CC_BIN="aarch64-linux-gnu-gcc"
        CXX_BIN="aarch64-linux-gnu-g++"
      fi
      CC_FLAGS=()
      LINK_FLAGS=(-shared)
      ;;
    *)
      echo "Unsupported platform: $PLATFORM" >&2
      exit 1
      ;;
  esac
}

function clone_grammar_source() {
  local source="$1"
  local ref="$2"
  local dest="$3"

  if [[ -n "$ref" ]]; then
    mkdir -p "$dest"
    if git -C "$dest" init -q >/dev/null 2>&1 &&
      git -C "$dest" remote add origin "$source" >/dev/null 2>&1 &&
      git -C "$dest" fetch --depth 1 origin "$ref" >/dev/null 2>&1 &&
      git -C "$dest" checkout --detach FETCH_HEAD >/dev/null 2>&1; then
      return 0
    fi

    rm -rf "$dest"
    echo "Failed to fetch pinned grammar ref ${ref} from ${source}" >&2
    return 1
  fi

  git clone --depth 1 "$source" "$dest" >/dev/null 2>&1
}

function find_grammar_dir() {
  local plugin_name="$1"
  local found=""

  if [[ -f "grammar.js" ]]; then
    printf '.\n'
    return 0
  fi

  if [[ -f "$plugin_name/grammar.js" ]]; then
    printf '%s\n' "$plugin_name"
    return 0
  fi

  if [[ -f "grammars/$plugin_name/grammar.js" ]]; then
    printf 'grammars/%s\n' "$plugin_name"
    return 0
  fi

  found="$(find . -maxdepth 3 -name grammar.js -print | head -1)"
  if [[ -n "$found" ]]; then
    dirname "$found"
  fi
}

function install_node_dependencies() {
  local install_dir="$1"

  if [[ ! -f "$install_dir/package.json" ]]; then
    return 0
  fi

  pushd "$install_dir" >/dev/null
  if [[ -f package-lock.json || -f npm-shrinkwrap.json ]]; then
    npm ci --ignore-scripts >/dev/null
    popd >/dev/null
    return 0
  fi

  echo "Refusing to install floating npm dependencies in ${install_dir} without a lockfile" >&2
  popd >/dev/null
  return 1
}

function find_locked_package_root() {
  local current_dir="$1"
  local repo_root="$2"

  while true; do
    if [[ -f "$current_dir/package.json" ]] &&
      [[ -f "$current_dir/package-lock.json" || -f "$current_dir/npm-shrinkwrap.json" ]]; then
      printf '%s\n' "$current_dir"
      return 0
    fi

    if [[ "$current_dir" == "$repo_root" || "$current_dir" == "/" ]]; then
      return 1
    fi

    current_dir="$(dirname "$current_dir")"
  done
}

function generate_parser_sources() {
  local repo_root="$1"

  if tree-sitter generate >/dev/null 2>&1; then
    return 0
  fi

  local install_dir
  install_dir="$(find_locked_package_root "$PWD" "$repo_root" || true)"
  if [[ -z "$install_dir" ]]; then
    echo "Refusing to install floating npm dependencies in $PWD without a lockfile" >&2
    return 1
  fi

  if ! install_node_dependencies "$install_dir"; then
    return 1
  fi

  tree-sitter generate >/dev/null || return 1
}

function build_grammar() {
  local plugin_name="$1"
  local toml_path="$2"
  local work_dir="$3"
  local out_dir="$4"

  local source
  source="$(grep 'source = ' "$toml_path" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
  local ref
  ref="$(grep 'ref = ' "$toml_path" | head -1 | sed 's/.*"\(.*\)".*/\1/' || true)"

  if [[ -z "$source" ]]; then
    echo "Missing grammar source for $plugin_name" >&2
    return 1
  fi

  local clone_dir="$work_dir/grammar-src-$plugin_name"
  clone_grammar_source "$source" "$ref" "$clone_dir" || return 1

  pushd "$clone_dir" >/dev/null || return 1
  local grammar_dir
  grammar_dir="$(find_grammar_dir "$plugin_name")"
  if [[ -z "$grammar_dir" ]]; then
    popd >/dev/null
    echo "Could not find grammar.js for $plugin_name" >&2
    return 1
  fi

  pushd "$grammar_dir" >/dev/null || return 1
  generate_parser_sources "$clone_dir" || return 1

  local src_dir="src"
  if [[ ! -f "$src_dir/parser.c" ]]; then
    local detected_src_dir
    local detected_src_path
    detected_src_path="$(find . -path '*/src/parser.c' -not -path '*/node_modules/*' -print | head -1)"
    if [[ -n "$detected_src_path" ]]; then
      detected_src_dir="$(dirname "$detected_src_path")"
    else
      detected_src_dir=""
    fi
    if [[ -n "$detected_src_dir" ]]; then
      src_dir="$detected_src_dir"
    fi
  fi

  if [[ ! -f "$src_dir/parser.c" ]]; then
    popd >/dev/null
    popd >/dev/null
    echo "parser.c not found for $plugin_name" >&2
    return 1
  fi

  "$CC_BIN" -c -fPIC -O2 -w "${CC_FLAGS[@]}" -I "$src_dir" "$src_dir/parser.c" -o parser.o
  local objects=("parser.o")

  if [[ -f "$src_dir/scanner.c" ]]; then
    "$CC_BIN" -c -fPIC -O2 -Wall "${CC_FLAGS[@]}" -I "$src_dir" "$src_dir/scanner.c" -o scanner.o || return 1
    objects+=("scanner.o")
  fi

  if [[ -f "$src_dir/scanner.cc" ]]; then
    "$CXX_BIN" -c -fPIC -O2 -Wall "${CC_FLAGS[@]}" -I "$src_dir" "$src_dir/scanner.cc" -o scanner_cc.o || return 1
    objects+=("scanner_cc.o")
    "$CXX_BIN" "${LINK_FLAGS[@]}" "${CC_FLAGS[@]}" "${objects[@]}" -o "$out_dir/$plugin_name.$GRAMMAR_EXT" || return 1
  else
    "$CC_BIN" "${LINK_FLAGS[@]}" "${CC_FLAGS[@]}" "${objects[@]}" -o "$out_dir/$plugin_name.$GRAMMAR_EXT" || return 1
  fi

  popd >/dev/null || return 1
  popd >/dev/null || return 1
}

function package_bundle() {
  local out_dir="$1"
  local bundle_dir="$2"

  mkdir -p "$bundle_dir"
  shopt -s nullglob
  for grammar in "$out_dir"/*."$GRAMMAR_EXT"; do
    local name
    name="$(basename "$grammar" ".$GRAMMAR_EXT")"
    mkdir -p "$bundle_dir/$name/grammars"
    cp "$grammar" "$bundle_dir/$name/grammars/$PLATFORM.$GRAMMAR_EXT"
  done
  shopt -u nullglob

  mkdir -p "$(dirname "$OUTPUT")"
  tar czf "$OUTPUT" -C "$bundle_dir" .
}

parse_args "$@"
configure_platform

if ! command -v tree-sitter >/dev/null 2>&1; then
  echo "tree-sitter CLI is required. Install it with scripts/install_tree_sitter_cli.sh or otherwise add it to PATH." >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$(mktemp -d)"
OUT_DIR="$WORK_DIR/grammars-out"
BUNDLE_DIR="$WORK_DIR/bundle"
trap 'rm -rf "$WORK_DIR"' EXIT

mkdir -p "$OUT_DIR"

declare -a FAILURES=()
for plugin_dir in "$ROOT_DIR"/plugins/*/; do
  plugin_name="$(basename "$plugin_dir")"
  toml_path="$plugin_dir/plugin.toml"

  if [[ ! -f "$toml_path" ]] || ! grep -q '\[grammar\]' "$toml_path"; then
    continue
  fi

  if ! build_grammar "$plugin_name" "$toml_path" "$WORK_DIR" "$OUT_DIR"; then
    FAILURES+=("$plugin_name")
  fi
done

if [[ ${#FAILURES[@]} -gt 0 ]]; then
  echo "Failed to build grammars for: ${FAILURES[*]}" >&2
  exit 1
fi

package_bundle "$OUT_DIR" "$BUNDLE_DIR"
