#!/bin/sh
# Remove three-finger drag. Leaves the input group and uinput udev rule in place.
EXT_UUID="disable-three-finger-swipe@daytonnolan.com"
systemctl --user disable --now three-finger-drag.service 2>/dev/null
rm -f "$HOME/.config/systemd/user/three-finger-drag.service"
systemctl --user daemon-reload
gnome-extensions disable "$EXT_UUID" 2>/dev/null
rm -rf "$HOME/.local/share/gnome-shell/extensions/$EXT_UUID" "$HOME/.local/share/three-finger-drag" "$HOME/.local/bin/three-finger-drag"
echo "Removed. To also revoke permissions:"
echo "  sudo gpasswd -d \$USER input"
echo "  sudo rm /etc/udev/rules.d/60-uinput.rules /etc/modules-load.d/uinput.conf"
