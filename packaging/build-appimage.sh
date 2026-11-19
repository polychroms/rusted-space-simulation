#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.0.3}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
APPDIR="${ROOT_DIR}/AppDir"
BIN_PATH="${ROOT_DIR}/target/release/rssim"
APPIMAGE_NAME="Rusted_Space_Simulation-${VERSION}-x86_64.AppImage"
OUT_APPIMAGE="${DIST_DIR}/${APPIMAGE_NAME}"
TOOL_CACHE="/tmp/appimagetool-x86_64.AppImage"

if [ ! -x "$BIN_PATH" ]; then
    cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"
fi

rm -rf "$APPDIR"
mkdir -p \
    "${APPDIR}/usr/bin" \
    "${APPDIR}/usr/share/applications" \
    "${APPDIR}/usr/share/icons/hicolor/256x256/apps" \
    "${APPDIR}/usr/share/icons/hicolor/scalable/apps" \
    "$DIST_DIR"

install -m 755 "$BIN_PATH" "${APPDIR}/usr/bin/rssim"
install -m 755 "${ROOT_DIR}/packaging/AppRun" "${APPDIR}/AppRun"
install -m 644 "${ROOT_DIR}/packaging/rssim.desktop" "${APPDIR}/rssim.desktop"
install -m 644 "${ROOT_DIR}/packaging/rssim.desktop" "${APPDIR}/usr/share/applications/rssim.desktop"
install -m 644 "${ROOT_DIR}/packaging/rssim.png" "${APPDIR}/rssim.png"
install -m 644 "${ROOT_DIR}/packaging/rssim.png" "${APPDIR}/.DirIcon"
install -m 644 "${ROOT_DIR}/packaging/rssim.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/rssim.png"
install -m 644 "${ROOT_DIR}/packaging/rssim.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/rssim.svg"

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "${APPDIR}/rssim.desktop"
fi

if ! command -v appimagetool >/dev/null 2>&1 && [ ! -x "$TOOL_CACHE" ]; then
    echo "Downloading appimagetool-x86_64.AppImage..."
    curl -fsSL --retry 4 --retry-delay 1 -o "$TOOL_CACHE" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" || \
    curl -4 -fsSL --retry 4 --retry-delay 1 -o "$TOOL_CACHE" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x "$TOOL_CACHE"
fi

TOOL_BIN="$(command -v appimagetool || echo "$TOOL_CACHE")"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL_BIN" "$APPDIR" "$OUT_APPIMAGE"
chmod +x "$OUT_APPIMAGE"
echo "Built AppImage: $OUT_APPIMAGE"
