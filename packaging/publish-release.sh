#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 3 ]; then
    echo "Usage: $0 <version_no_v> <title> <notes_file>" >&2
    exit 1
fi

VER="$1"
TITLE="$2"
NOTES_FILE="$3"
TAG="v${VER}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

grep -q "^version = \"${VER}\"" Cargo.toml || {
    echo "Error: Cargo.toml version is not ${VER}" >&2
    exit 1
}
grep -q "^VERSION=\"${TAG}\"" install.sh || {
    echo "Error: install.sh VERSION is not ${TAG}" >&2
    exit 1
}
grep -q "Rusted Space Simulation (rssim) ${TAG}" src/main.rs || {
    echo "Error: src/main.rs help header does not mention ${TAG}" >&2
    exit 1
}

FORBIDDEN_WORD="sand""box"
if rg -n -i "$FORBIDDEN_WORD" src/ Cargo.toml README.md install.sh packaging/; then
    echo "Error: forbidden naming term found in repository files!" >&2
    exit 1
fi

cargo clippy --release --all-targets -- -D warnings
cargo test --release
cargo build --release
cp target/release/rssim /home/radu/bin/rssim
/home/radu/bin/rssim --bench

bash packaging/build-appimage.sh "$VER"
rm -rf AppDir

APPIMAGE="dist/Rusted_Space_Simulation-${VER}-x86_64.AppImage"
TMP_AI="/tmp/rssim_check_${VER}.AppImage"
cp "$APPIMAGE" "$TMP_AI"
chmod +x "$TMP_AI"
APPIMAGELAUNCHER_DISABLE=1 APPIMAGE_EXTRACT_AND_RUN=1 "$TMP_AI" --help >/dev/null
APPIMAGELAUNCHER_DISABLE=1 APPIMAGE_EXTRACT_AND_RUN=1 "$TMP_AI" --bench >/dev/null
rm -f "$TMP_AI"

PKG_NAME="rssim-${TAG}-x86_64-linux"
PKG_DIR="dist/${PKG_NAME}"
rm -rf "$PKG_DIR"
mkdir -p "${PKG_DIR}/packaging"
cp target/release/rssim "${PKG_DIR}/rssim"
cp target/release/rssim dist/rssim
cp LICENSE README.md install.sh "${PKG_DIR}/"
cp install.sh dist/install.sh
cp packaging/rssim.desktop packaging/rssim.png packaging/rssim.svg "${PKG_DIR}/packaging/"
chmod +x "${PKG_DIR}/rssim" "${PKG_DIR}/install.sh" dist/rssim dist/install.sh

tar -C dist -czf "dist/${PKG_NAME}.tar.gz" "$PKG_NAME"
rm -rf "$PKG_DIR"

(
    cd dist
    sha256sum \
        "${PKG_NAME}.tar.gz" \
        "Rusted_Space_Simulation-${VER}-x86_64.AppImage" \
        rssim \
        install.sh > SHA256SUMS.txt
)

git add -A
git status -s
git commit -m "$TITLE"
git tag -a "$TAG" -m "$TITLE"
git push origin main
git push origin "$TAG"

gh release create "$TAG" \
    "$APPIMAGE" \
    "dist/${PKG_NAME}.tar.gz" \
    dist/rssim \
    dist/install.sh \
    dist/SHA256SUMS.txt \
    --repo rappar222/rusted-space-simulation \
    --title "$TITLE" \
    --notes-file "$NOTES_FILE"

gh release view "$TAG" --repo rappar222/rusted-space-simulation --json tagName,url,assets
