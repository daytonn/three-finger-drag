#!/bin/sh
# Rebuild the venv if a system Python upgrade broke it or evdev is missing.
D="$(dirname "$0")"
"$D/venv/bin/python" -c 'import evdev' 2>/dev/null && exit 0
rm -rf "$D/venv"
python3 -m venv "$D/venv" && "$D/venv/bin/pip" install -q evdev
