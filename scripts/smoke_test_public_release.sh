#!/usr/bin/env bash
set -euo pipefail

repo="yshaaban/sentrux"
version=""
install_dir=""
max_attempts="30"
sleep_seconds="3"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
  echo "Usage: $0 --version <tag> [--repo <owner/name>] [--install-dir <dir>] [--max-attempts <count>] [--sleep-seconds <count>]" >&2
  exit 1
}

function parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --repo)
        repo="${2:-}"
        shift 2
        ;;
      --version)
        version="${2:-}"
        shift 2
        ;;
      --install-dir)
        install_dir="${2:-}"
        shift 2
        ;;
      --max-attempts)
        max_attempts="${2:-}"
        shift 2
        ;;
      --sleep-seconds)
        sleep_seconds="${2:-}"
        shift 2
        ;;
      *)
        usage
        ;;
    esac
  done

  if [[ -z "$version" ]]; then
    usage
  fi
}

function latest_release_tag() {
  curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" | sed -n 's/.*"tag_name": "\(.*\)".*/\1/p' | head -1
}

function normalized_tag_version() {
  printf '%s\n' "${1#v}"
}

function wait_for_latest_release() {
  local attempt latest_tag

  for ((attempt = 1; attempt <= max_attempts; attempt += 1)); do
    latest_tag="$(latest_release_tag || true)"
    if [[ "$latest_tag" == "$version" ]]; then
      return 0
    fi

    sleep "$sleep_seconds"
  done

  echo "Latest release for ${repo} never reached ${version}" >&2
  return 1
}

function main() {
  parse_args "$@"

  local tmpdir home_dir repo_dir bin_dir installed_bin version_output expected_version
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  bin_dir="${install_dir:-$tmpdir/bin}"
  home_dir="$tmpdir/home"
  repo_dir="$tmpdir/repo"
  installed_bin="$bin_dir/sentrux"

  mkdir -p "$bin_dir" "$home_dir" "$repo_dir"

  wait_for_latest_release

  SENTRUX_INSTALL_REPO="$repo" \
  SENTRUX_INSTALL_VERSION="$version" \
  SENTRUX_INSTALL_DIR="$bin_dir" \
  "$repo_root/install.sh"

  if [[ ! -x "$installed_bin" ]]; then
    echo "Public release smoke failed: installed binary missing at ${installed_bin}" >&2
    exit 1
  fi

  version_output="$("$installed_bin" --version | head -1)"
  expected_version="$(normalized_tag_version "$version")"
  if [[ "$version_output" != "sentrux ${expected_version}" ]]; then
    echo "Installed binary version mismatch: expected sentrux ${expected_version}, got ${version_output}" >&2
    exit 1
  fi

  cat >"$repo_dir/example.js" <<'EOF'
function publicReleaseSmoke(input) {
  if (input) {
    return 1;
  }
  return 0;
}
EOF

  HOME="$home_dir" "$installed_bin" brief --mode repo-onboarding "$repo_dir" >/dev/null
  echo "Public release smoke test passed for ${repo} ${version}"
}

main "$@"
