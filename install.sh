#!/bin/sh
set -e

REPO="${SENTRUX_INSTALL_REPO:-yshaaban/sentrux}"
INSTALL_DIR="${SENTRUX_INSTALL_DIR:-/usr/local/bin}"
VERSION="${SENTRUX_INSTALL_VERSION:-}"
API_URL="${SENTRUX_INSTALL_API_URL:-https://api.github.com/repos/${REPO}/releases/latest}"
BASE_URL="${SENTRUX_INSTALL_BASE_URL:-https://github.com/${REPO}/releases/download}"
INSTALL_OS="${SENTRUX_INSTALL_OS:-}"
INSTALL_ARCH="${SENTRUX_INSTALL_ARCH:-}"
ARTIFACT="${SENTRUX_INSTALL_ARTIFACT:-}"

if [ -z "${VERSION}" ]; then
    VERSION=$(curl -fsSL "${API_URL}" | grep '"tag_name"' | sed 's/.*"tag_name": "//;s/".*//')
    if [ -z "$VERSION" ]; then
        echo "Error: could not detect latest version"
        exit 1
    fi
fi

OS="${INSTALL_OS:-$(uname -s)}"
ARCH="${INSTALL_ARCH:-$(uname -m)}"

if [ -z "${ARTIFACT}" ]; then
    case "${OS}" in
        Darwin)
            case "${ARCH}" in
                arm64|aarch64) ARTIFACT="sentrux-darwin-arm64" ;;
                x86_64)
                    echo "Error: macOS Intel (x86_64) binary not available yet."
                    echo "Build from source: git clone https://github.com/${REPO} && cd sentrux && cargo build --release"
                    exit 1
                    ;;
                *) echo "Error: unsupported architecture: ${ARCH}"; exit 1 ;;
            esac
            ;;
        Linux)
            case "${ARCH}" in
                x86_64) ARTIFACT="sentrux-linux-x86_64" ;;
                aarch64|arm64) ARTIFACT="sentrux-linux-aarch64" ;;
                *) echo "Error: unsupported architecture: ${ARCH}"; exit 1 ;;
            esac
            ;;
        *)
            echo "Error: unsupported OS: ${OS}"
            exit 1
            ;;
    esac
fi

URL="${BASE_URL}/${VERSION}/${ARTIFACT}"

echo "Installing sentrux ${VERSION} (${OS} ${ARCH})..."
echo "Downloading ${URL}"

TMP=$(mktemp)
if command -v curl > /dev/null 2>&1; then
    curl -fsSL "${URL}" -o "${TMP}"
elif command -v wget > /dev/null 2>&1; then
    wget -qO "${TMP}" "${URL}"
else
    echo "Error: curl or wget required"
    exit 1
fi

chmod +x "${TMP}"

if [ -w "${INSTALL_DIR}" ]; then
    mv "${TMP}" "${INSTALL_DIR}/sentrux"
else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "${TMP}" "${INSTALL_DIR}/sentrux"
fi

echo "sentrux installed to ${INSTALL_DIR}/sentrux"
echo ""
echo "Run:  sentrux              # GUI mode"
echo "      sentrux mcp          # MCP server for AI agents"
echo "      sentrux gate .       # patch safety against a saved baseline"
echo "      sentrux brief --mode patch ."
