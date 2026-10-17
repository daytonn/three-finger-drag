#!/bin/sh
# Install three-finger drag for the current user. Prompts for sudo once.
# Downloads the prebuilt binary for this version; pass --build to compile
# from source instead (also the fallback if the download fails).
set -e
SRC="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin"
REPO="daytonn/three-finger-drag"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -1)"
BUILD=0
[ "$1" = "--build" ] && BUILD=1
EXT_UUID="disable-three-finger-swipe@daytonnolan.com"
EXT_DIR="$HOME/.local/share/gnome-shell/extensions/$EXT_UUID"

echo "==> System permissions (sudo)"
sudo usermod -aG input "$USER"
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' |
    sudo tee /etc/udev/rules.d/60-uinput.rules >/dev/null
echo uinput | sudo tee /etc/modules-load.d/uinput.conf >/dev/null
sudo modprobe uinput 2>/dev/null || true
sudo udevadm control --reload
sudo udevadm trigger --name-match=uinput

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Fetch and verify the release binary into $TMP; fails if unavailable
download() {
    asset="three-finger-drag-$(uname -m)-linux.tar.gz"
    url="https://github.com/$REPO/releases/download/v$VERSION/$asset"
    echo "Downloading $url"
    curl -fsSL -o "$TMP/$asset" "$url" &&
        curl -fsSL -o "$TMP/$asset.sha256" "$url.sha256" &&
        (cd "$TMP" && sha256sum -c --quiet "$asset.sha256") &&
        tar -C "$TMP" -xzf "$TMP/$asset" three-finger-drag
}

build() {
    if ! command -v cargo >/dev/null; then
        echo "cargo not found. Install Rust first (e.g. 'asdf install' in this directory)." >&2
        exit 1
    fi
    (cd "$SRC" && cargo build --release)
    cp "$SRC/target/release/three-finger-drag" "$TMP/"
}

echo "==> Daemon v$VERSION"
if [ "$BUILD" = 1 ]; then
    build
elif ! download; then
    echo "No prebuilt binary available; building from source."
    build
fi
mkdir -p "$BIN"
systemctl --user stop three-finger-drag.service 2>/dev/null || true
install -m755 "$TMP/three-finger-drag" "$BIN/"
rm -rf "$HOME/.local/share/three-finger-drag"  # old Python version

echo "==> systemd user service"
mkdir -p "$HOME/.config/systemd/user"
cp "$SRC/three-finger-drag.service" "$HOME/.config/systemd/user/"
systemctl --user daemon-reload
systemctl --user enable three-finger-drag.service
systemctl --user restart three-finger-drag.service 2>/dev/null || true

echo "==> GNOME extension"
mkdir -p "$EXT_DIR"
cp "$SRC/extension/$EXT_UUID/"* "$EXT_DIR/"
current="$(gsettings get org.gnome.shell enabled-extensions)"
case "$current" in
    *"$EXT_UUID"*) ;;
    "@as []") gsettings set org.gnome.shell enabled-extensions "['$EXT_UUID']" ;;
    *) gsettings set org.gnome.shell enabled-extensions "${current%]}, '$EXT_UUID']" ;;
esac

echo
echo "Done. Log out and back in to finish (needed for the input group and the extension)."
