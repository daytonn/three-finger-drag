#!/bin/sh
# Install three-finger drag for the current user. Prompts for sudo once.
set -e
SRC="$(cd "$(dirname "$0")" && pwd)"
APP="$HOME/.local/share/three-finger-drag"
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
mkdir -p "$APP"
cp "$SRC/three_finger_drag.py" "$SRC/ensure-venv.sh" "$APP/"
chmod +x "$APP/ensure-venv.sh"
"$APP/ensure-venv.sh"

echo "==> systemd user service"
mkdir -p "$HOME/.config/systemd/user"
cp "$SRC/three-finger-drag.service" "$HOME/.config/systemd/user/"
systemctl --user daemon-reload
systemctl --user enable three-finger-drag.service

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
