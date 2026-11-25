# CosmicArchUpdates

A [COSMIC](https://system76.com/cosmic) desktop panel applet for Arch Linux that shows how many package updates are available.

- The panel shows an update icon and the number of pending updates (`0` when up to date).
- Click it to see "N updates available" (or "0 updates available"), then update with one button.
- Choose whether to count **Pacman** (official repos), the **AUR**, or **both**.

## Features

| Feature | Details |
| --- | --- |
| Panel count | Icon + number of pending updates, for horizontal and vertical panels |
| Popup | Headline count, per-source breakdown when both are selected, **Update now** and **Check now** buttons |
| Source setting | Pacman, AUR, or Pacman + AUR |
| Interval setting | Check every 5 min, 15 min, 30 min (default), 1 h or 2 h |
| Update action | Runs the update in a terminal window, then re-checks when it closes |

Settings are saved with `cosmic-config` (app id `com.example.CosmicAppletArchUpdates`).

## Requirements

- Arch Linux (or derivative) with the COSMIC desktop
- Rust toolchain (`cargo`) to build
- [`pacman-contrib`](https://archlinux.org/packages/extra/x86_64/pacman-contrib/) – provides `checkupdates`, which counts repo updates without root
- `paru` or `yay` – only needed for AUR support
- A terminal emulator for the update action: `cosmic-term`, `kitty`, `alacritty`, `foot`, `konsole`, `gnome-terminal` or `xterm` (first one found is used)

## Install

```sh
git clone https://github.com/kevinRForshey/CosmicArchUpdates.git
cd CosmicArchUpdates
scripts/install.sh
```

This builds a release binary and installs it to `~/.local/bin/cosmic-applet-arch-updates`, plus a desktop entry in `~/.local/share/applications/`.

Then add it to the panel: **COSMIC Settings → Desktop → Panel → (Start/Center/End) → Add applet → Arch Updates**. If it doesn't appear, restart `cosmic-panel` or log out and back in.

To remove it: `scripts/install.sh --uninstall`.

## How it works

- **Counting:** `checkupdates` for the official repos; `paru -Qua` (or `yay -Qua`) for the AUR. Each output line is one pending update.
- **Updating:**
  - Pacman only: `sudo pacman -Syu`
  - AUR only: `<helper> -Sua`
  - Both: `<helper> -Syu` (falls back to `sudo pacman -Syu` if no helper is installed)
- **Errors** (for example a missing `checkupdates` or AUR helper) are shown in the popup instead of silently reporting 0.

## Development

```sh
cargo build --release
cargo test
```

Layout: `src/updates.rs` (checking and update commands, no GUI dependencies), `src/config.rs` (persisted settings), `src/app.rs` (the libcosmic applet), `data/` (desktop entry), `scripts/install.sh`.

## License

Apache-2.0. See [LICENSE](LICENSE).
