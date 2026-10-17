# three-finger-drag

macOS-style three-finger drag for Linux touchpads on GNOME, working on **Wayland** and X11.

Put three fingers on the trackpad and move them: the left mouse button is held and the pointer follows your fingers. Lift your fingers to drop. Use it to select text, move windows, drag files, and so on.

## How it works

There are two parts:

1. **`three_finger_drag.py`** is a small daemon that reads the touchpad's raw multitouch events through evdev. Once three fingers have moved past a small threshold, it holds the left button and moves the pointer on a virtual mouse (uinput). libinput treats three-finger contact as a gesture rather than pointer motion, so the pointer never moves twice.
2. **The `disable-three-finger-swipe@daytonnolan.com` GNOME Shell extension** turns off GNOME's built-in three-finger swipes (workspace switching and the overview) so they don't fire during a drag. Four-finger gestures and two-finger scrolling are unchanged.

The daemon runs as a systemd user service that starts with your graphical session.

## Requirements

- GNOME Shell 46 or newer (tested on Zorin OS 18 / GNOME 46, Wayland)
- Python 3 with `venv`, plus `gcc` and Python headers to build `python-evdev` (`sudo apt install python3-venv python3-dev gcc`)
- A multitouch touchpad that reports three-finger contact (`BTN_TOOL_TRIPLETAP`)

## Install

```sh
git clone git@github.com:daytonn/three-finger-drag.git
cd three-finger-drag
./install.sh
```

Then **log out and back in**. Check that it's running:

```sh
systemctl --user status three-finger-drag
```

`install.sh` does the following:

- adds you to the `input` group so the daemon can read the touchpad
- installs a udev rule that makes `/dev/uinput` writable by the `input` group
- copies the daemon to `~/.local/share/three-finger-drag` and builds its virtualenv
- installs and enables the systemd user service
- installs and enables the GNOME extension

> **Note:** Members of the `input` group can read all input devices, including keyboards. This is normal for gesture tools, but be aware of it.

## Configuration

Add options to the end of the `ExecStart=` line in `~/.config/systemd/user/three-finger-drag.service`, then run:

```sh
systemctl --user daemon-reload && systemctl --user restart three-finger-drag
```

| Option | Default | Description |
| --- | --- | --- |
| `--speed N` | `4.0` | Pointer units per mm of finger travel |
| `--threshold MM` | `1.0` | Movement needed before the drag starts, so three-finger taps still work |
| `--release-delay S` | `0` | Seconds to keep the button held after lifting, so you can reposition your fingers mid-drag |

## Uninstall

```sh
./uninstall.sh
```

This leaves the `input` group membership and the udev rule in place, and prints the commands to remove them.

## Troubleshooting

- **`"/dev/uinput" cannot be opened for writing`** means the udev rule hasn't been applied yet. Run `sudo udevadm trigger --name-match=uinput` or reboot.
- **GNOME's three-finger swipes are back after a GNOME upgrade** means GNOME probably renamed the internals the extension relies on. The drag still works, but the extension needs updating.
- **The service fails after an OS upgrade.** `ensure-venv.sh` runs before each start and rebuilds the virtualenv if a Python upgrade broke it. Check `journalctl --user -u three-finger-drag` for details.

## License

MIT
