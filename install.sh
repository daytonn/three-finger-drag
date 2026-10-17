#!/bin/sh
# Install three-finger drag for the current user. Prompts for sudo once.
set -e
SRC="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin"
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

echo "==> Daemon"
if ! command -v cargo >/dev/null; then
    echo "cargo not found. Install Rust first (e.g. 'asdf install' in this directory)." >&2
    exit 1
fi
(cd "$SRC" && cargo build --release)
mkdir -p "$BIN"
systemctl --user stop three-finger-drag.service 2>/dev/null || true
install -m755 "$SRC/target/release/three-finger-drag" "$BIN/"
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
