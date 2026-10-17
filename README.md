# three-finger-drag

macOS-style three-finger drag for Linux touchpads on GNOME, working on **Wayland** and X11.

Put three fingers on the trackpad and move them: the left mouse button is held and the pointer follows your fingers. Lift your fingers to drop. Use it to select text, move windows, drag files, and so on.

## How it works

There are two parts:

1. **`three-finger-drag`** is a small Rust daemon that reads the touchpad's raw multitouch events through evdev. Once three fingers have moved past a small threshold, it holds the left button and moves the pointer on a virtual mouse (uinput). libinput treats three-finger contact as a gesture rather than pointer motion, so the pointer never moves twice.
2. **The `disable-three-finger-swipe@daytonnolan.com` GNOME Shell extension** turns off GNOME's built-in three-finger swipes (workspace switching and the overview) so they don't fire during a drag. Four-finger gestures and two-finger scrolling are unchanged.

The daemon runs as a systemd user service that starts with your graphical session.

## Requirements

- GNOME Shell 46 or newer (tested on Zorin OS 18 / GNOME 46, Wayland)
- `curl` (prebuilt binaries are provided for x86_64 and aarch64)
- Rust, only if you build from source. The version is pinned in `.tool-versions`, so with [asdf](https://asdf-vm.com) run `asdf plugin add rust && asdf install` in the repo.
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
- downloads the prebuilt daemon for this version from GitHub Releases, verifies its checksum, and installs it to `~/.local/bin/three-finger-drag`. If no binary is available it builds from source instead; run `./install.sh --build` to always build from source.
- installs and enables the systemd user service
- installs and enables the GNOME extension

> **Note:** Members of the `input` group can read all input devices, including keyboards. This is normal for gesture tools, but be aware of it.

## Development

```sh
cargo build --release
install -m755 target/release/three-finger-drag ~/.local/bin/
systemctl --user restart three-finger-drag
```

### Releasing

Bump `version` in `Cargo.toml`, commit, then tag and push:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

The [Release workflow](.github/workflows/release.yml) builds static (musl) binaries for x86_64 and aarch64 and publishes them, with SHA-256 checksums, as a GitHub release. The tag must match the `Cargo.toml` version.

## Configuration

Add options to the end of the `ExecStart=` line in `~/.config/systemd/user/three-finger-drag.service`, then run:

```sh
systemctl --user daemon-reload && systemctl --user restart three-finger-drag
```

| Option | Default | Description |
| --- | --- | --- |
| `--speed N` | `4.0` | Pointer units per mm of finger travel |
| `--threshold MM` | `1.0` | Movement needed before the drag starts, so three-finger taps still work |
| `--release-delay S` | `0.5` | Seconds to keep the button held after lifting, so you can reposition your fingers mid-drag. Set to `0` to drop immediately |

## Uninstall

```sh
./uninstall.sh
```

This leaves the `input` group membership and the udev rule in place, and prints the commands to remove them.

## Troubleshooting

- **`"/dev/uinput" cannot be opened for writing`** means the udev rule hasn't been applied yet. Run `sudo udevadm trigger --name-match=uinput` or reboot.
- **GNOME's three-finger swipes are back after a GNOME upgrade** means GNOME probably renamed the internals the extension relies on. The drag still works, but the extension needs updating.
- **The service isn't running.** Check `journalctl --user -u three-finger-drag` for details.

## License

MIT
