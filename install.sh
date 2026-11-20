#!/usr/bin/env bash
set -euo pipefail

REPO="rappar222/rusted-space-simulation"
VERSION="v0.1.2"
INSTALL_DIR="${INSTALL_DIR:-}"
FROM_SOURCE=0

usage() {
    cat <<'EOF'
Rusted Space Simulation Installer

Usage:
  install.sh [options]

Options:
  --version <tag>   GitHub release tag to install (default: v0.1.2)
  --dir <path>      Target bin directory (default: ~/bin, or /usr/local/bin if root)
  --from-source     Download GitHub source archive and build with cargo --release
  -h, --help        Show this help message
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:?missing value for --version}"
            shift 2
            ;;
        --dir)
            INSTALL_DIR="${2:?missing value for --dir}"
            shift 2
            ;;
        --from-source)
            FROM_SOURCE=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

fetch_file() {
    local url="$1"
    local dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fL --retry 4 --retry-delay 1 -o "$dest" "$url" \
            || curl -4 -fL --retry 4 --retry-delay 1 -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$dest" "$url"
    else
        echo "Error: curl or wget is required to download files from GitHub." >&2
        exit 1
    fi
}

if [ -n "$INSTALL_DIR" ]; then
    BIN_DIR="$INSTALL_DIR"
elif [ "$(id -u)" -eq 0 ]; then
    BIN_DIR="/usr/local/bin"
else
    BIN_DIR="$HOME/bin"
fi

mkdir -p "$BIN_DIR"

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="${DATA_HOME}/applications"
ICON_256_DIR="${DATA_HOME}/icons/hicolor/256x256/apps"
ICON_SVG_DIR="${DATA_HOME}/icons/hicolor/scalable/apps"
SHARE_DIR="${DATA_HOME}/rusted-space-simulation"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

ARCH="$(uname -m)"
OS="$(uname -s)"
VER_NO_V="${VERSION#v}"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
TAR_NAME="rssim-${VERSION}-x86_64-linux.tar.gz"
APPIMAGE_NAME="Rusted_Space_Simulation-${VER_NO_V}-x86_64.AppImage"

if [ "$FROM_SOURCE" -eq 0 ] && [ "$OS" = "Linux" ] && [ "$ARCH" = "x86_64" ]; then
    echo "Downloading ${REPO} ${VERSION} release files from GitHub..."
    fetch_file "${BASE_URL}/${TAR_NAME}" "${TMP_DIR}/${TAR_NAME}"
    fetch_file "${BASE_URL}/${APPIMAGE_NAME}" "${TMP_DIR}/${APPIMAGE_NAME}"

    if fetch_file "${BASE_URL}/SHA256SUMS.txt" "${TMP_DIR}/SHA256SUMS.txt" 2>/dev/null; then
        if command -v sha256sum >/dev/null 2>&1; then
            (cd "$TMP_DIR" && sha256sum -c --ignore-missing SHA256SUMS.txt)
        fi
    fi

    tar -xzf "${TMP_DIR}/${TAR_NAME}" -C "$TMP_DIR"
    SRC_ROOT="${TMP_DIR}/rssim-${VERSION}-x86_64-linux"

    install -m 755 "${SRC_ROOT}/rssim" "${BIN_DIR}/rssim"
    install -m 755 "${TMP_DIR}/${APPIMAGE_NAME}" "${BIN_DIR}/${APPIMAGE_NAME}"

    mkdir -p "$APP_DIR" "$ICON_256_DIR" "$ICON_SVG_DIR" "$SHARE_DIR"
    [ -f "${SRC_ROOT}/README.md" ] && install -m 644 "${SRC_ROOT}/README.md" "${SHARE_DIR}/README.md"
    [ -f "${SRC_ROOT}/LICENSE" ] && install -m 644 "${SRC_ROOT}/LICENSE" "${SHARE_DIR}/LICENSE"
    [ -f "${SRC_ROOT}/packaging/rssim.png" ] && install -m 644 "${SRC_ROOT}/packaging/rssim.png" "${ICON_256_DIR}/rssim.png"
    [ -f "${SRC_ROOT}/packaging/rssim.svg" ] && install -m 644 "${SRC_ROOT}/packaging/rssim.svg" "${ICON_SVG_DIR}/rssim.svg"
    if [ -f "${SRC_ROOT}/packaging/rssim.desktop" ]; then
        sed "s|^Exec=.*|Exec=${BIN_DIR}/rssim|" "${SRC_ROOT}/packaging/rssim.desktop" > "${APP_DIR}/rssim.desktop"
        chmod 644 "${APP_DIR}/rssim.desktop"
    fi
else
    echo "Downloading source archive ${VERSION} from GitHub and building with cargo..."
    if ! command -v cargo >/dev/null 2>&1 || ! command -v glslc >/dev/null 2>&1; then
        echo "Error: cargo and glslc are required for source builds." >&2
        exit 1
    fi
    SRC_TAR="${TMP_DIR}/src.tar.gz"
    fetch_file "https://github.com/${REPO}/archive/refs/tags/${VERSION}.tar.gz" "$SRC_TAR"
    tar -xzf "$SRC_TAR" -C "$TMP_DIR"
    BUILD_DIR="$(find "$TMP_DIR" -maxdepth 1 -mindepth 1 -type d | head -n 1)"
    cargo build --release --manifest-path "${BUILD_DIR}/Cargo.toml"
    install -m 755 "${BUILD_DIR}/target/release/rssim" "${BIN_DIR}/rssim"

    mkdir -p "$APP_DIR" "$ICON_256_DIR" "$ICON_SVG_DIR" "$SHARE_DIR"
    [ -f "${BUILD_DIR}/README.md" ] && install -m 644 "${BUILD_DIR}/README.md" "${SHARE_DIR}/README.md"
    [ -f "${BUILD_DIR}/LICENSE" ] && install -m 644 "${BUILD_DIR}/LICENSE" "${SHARE_DIR}/LICENSE"
    [ -f "${BUILD_DIR}/packaging/rssim.png" ] && install -m 644 "${BUILD_DIR}/packaging/rssim.png" "${ICON_256_DIR}/rssim.png"
    [ -f "${BUILD_DIR}/packaging/rssim.svg" ] && install -m 644 "${BUILD_DIR}/packaging/rssim.svg" "${ICON_SVG_DIR}/rssim.svg"
    if [ -f "${BUILD_DIR}/packaging/rssim.desktop" ]; then
        sed "s|^Exec=.*|Exec=${BIN_DIR}/rssim|" "${BUILD_DIR}/packaging/rssim.desktop" > "${APP_DIR}/rssim.desktop"
        chmod 644 "${APP_DIR}/rssim.desktop"
    fi
fi

if [ "$(id -u)" -ne 0 ] && [ "$BIN_DIR" = "$HOME/bin" ] && ! echo ":$PATH:" | grep -q ":$HOME/bin:"; then
    if echo ":$PATH:" | grep -q ":$HOME/.local/bin:"; then
        mkdir -p "$HOME/.local/bin"
        ln -sf "${BIN_DIR}/rssim" "$HOME/.local/bin/rssim"
        echo "Also linked $HOME/.local/bin/rssim -> ${BIN_DIR}/rssim (already in PATH)."
    else
        echo "Note: add '$BIN_DIR' to your PATH to run 'rssim' from anywhere:"
        echo "  export PATH=\"$BIN_DIR:\$PATH\""
    fi
fi

echo "Installed Rusted Space Simulation (${VERSION}) to: ${BIN_DIR}"
ls -lh "${BIN_DIR}/rssim" "${BIN_DIR}/${APPIMAGE_NAME}" 2>/dev/null || ls -lh "${BIN_DIR}/rssim"
