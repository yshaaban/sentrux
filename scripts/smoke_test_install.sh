#!/usr/bin/env bash
set -euo pipefail

artifact_path=""
artifact_name=""
grammar_bundle_path=""
version="v-smoke-test"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
  echo "Usage: $0 --artifact-path <path> --artifact-name <name> [--grammar-bundle-path <path>] [--version <version>]" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifact-path)
      artifact_path="${2:-}"
      shift 2
      ;;
    --artifact-name)
      artifact_name="${2:-}"
      shift 2
      ;;
    --grammar-bundle-path)
      grammar_bundle_path="${2:-}"
      shift 2
      ;;
    --version)
      version="${2:-}"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

if [[ -z "$artifact_path" || -z "$artifact_name" ]]; then
  usage
fi

tmpdir="$(mktemp -d)"
release_dir="$tmpdir/releases/download/$version"
install_dir="$tmpdir/install/bin"
runtime_home="$tmpdir/runtime-home"
runtime_repo="$tmpdir/runtime-repo"
server_log="$tmpdir/http.log"
mkdir -p "$release_dir" "$install_dir"
cp "$artifact_path" "$release_dir/$artifact_name"
chmod +x "$release_dir/$artifact_name"

port="$(
  python3 - <<'PY'
import socket

sock = socket.socket()
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
)"

cleanup() {
  if [[ -n "${server_pid:-}" ]]; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$tmpdir"
}

trap cleanup EXIT

python3 -m http.server "$port" --bind 127.0.0.1 --directory "$tmpdir" >"$server_log" 2>&1 &
server_pid=$!
sleep 1

SENTRUX_INSTALL_VERSION="$version" \
SENTRUX_INSTALL_BASE_URL="http://127.0.0.1:$port/releases/download" \
SENTRUX_INSTALL_ARTIFACT="$artifact_name" \
SENTRUX_INSTALL_DIR="$install_dir" \
"$repo_root/install.sh"

installed_bin="$install_dir/sentrux"
if [[ ! -x "$installed_bin" ]]; then
  echo "Smoke test failed: installed binary missing at $installed_bin" >&2
  exit 1
fi

"$installed_bin" --version >/dev/null

if [[ -n "$grammar_bundle_path" ]]; then
  mkdir -p "$runtime_home/.sentrux/plugins" "$runtime_repo"
  tar xzf "$grammar_bundle_path" -C "$runtime_home/.sentrux/plugins"
  cat >"$runtime_repo/example.js" <<'EOF'
function smokeTest(input) {
  if (input) {
    return 1;
  }
  return 0;
}
EOF

  HOME="$runtime_home" SENTRUX_SKIP_GRAMMAR_DOWNLOAD=1 \
    "$installed_bin" brief --mode repo-onboarding "$runtime_repo" >/dev/null
fi

echo "Install smoke test passed for $artifact_name"
