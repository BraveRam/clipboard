# Clipboard

> A fast, keyboard-driven clipboard history for Linux.

A small standalone desktop app that quietly tracks everything you copy — text and images — and lets you search and paste your history through a Raycast-style overlay summoned by a custom keyboard shortcut.

```
┌──────────────────────────────────────────────────────┐
│ ⌕  Search clipboard…                       4 items │
├──────────────────────────────────────────────────────┤
│ 📌  git rebase -i HEAD~3                            │
│ 📌  https://tauri.app/v2/                           │
├──────────────────────────────────────────────────────┤
│  ¶  Lorem ipsum dolor sit amet…                     │
│  🖼  Image · 1920×1080 · 2.4 MB                      │
└──────────────────────────────────────────────────────┘
   ↑↓ navigate   ↵ paste   Ctrl+P pin   Esc close
```

Built with [Tauri 2](https://tauri.app), React 19, and SQLite. Runs as a single-instance daemon — bind any key combo in your desktop's keyboard settings; pressing it again toggles the existing window instead of spawning a duplicate.

## Features

- **Text + image capture** — automatic, deduplicated by SHA-256.
- **50-item rolling history** — pinned items are stored separately and never evicted.
- **Fuzzy search** across the whole history as you type.
- **Keyboard-first** — arrow keys to navigate, Enter to paste back, Ctrl+P to pin, Ctrl+⌫ to delete, Esc to dismiss.
- **Privacy-respecting** — everything stays in `~/.config/com.plxor.clipboard/`. No network calls, ever.
- **Single binary** — ships as a self-contained AppImage that bundles GTK and WebKit so it runs on any modern Linux distro.

## Install

Grab the AppImage from the [latest release](../../releases/latest):

```bash
mkdir -p ~/Applications
mv ~/Downloads/Clipboard_*.AppImage ~/Applications/Clipboard.AppImage
chmod +x ~/Applications/Clipboard.AppImage
```

Then bind it to a keyboard shortcut:

| Desktop  | Where                                                                                |
| -------- | ------------------------------------------------------------------------------------ |
| GNOME    | Settings → Keyboard → Custom Shortcuts → `+`; Command: `~/Applications/Clipboard.AppImage` |
| KDE      | System Settings → Shortcuts → Add Custom Shortcut → Command/URL                      |
| Sway/i3  | `bindsym $mod+v exec ~/Applications/Clipboard.AppImage`                              |
| Hyprland | `bind = SUPER, V, exec, ~/Applications/Clipboard.AppImage`                           |

The **first** press of the shortcut launches the daemon and shows the overlay. **Subsequent** presses toggle the overlay's visibility on the existing daemon — `tauri-plugin-single-instance` forwards the launch and the new process exits immediately.

### Wayland users

Install `wl-clipboard` so the underlying clipboard library can read the Wayland selection:

```bash
sudo dnf install wl-clipboard       # Fedora
sudo apt install wl-clipboard       # Debian / Ubuntu
sudo pacman -S wl-clipboard         # Arch
```

## Usage

| Key       | Action            |
| --------- | ----------------- |
| `↑` `↓`   | Navigate          |
| `↵`       | Paste back & hide |
| `Ctrl+P`  | Toggle pin        |
| `Ctrl+⌫`  | Delete entry      |
| `Esc`     | Hide overlay      |

After pressing `↵`, the selected entry is on your system clipboard — paste it normally with `Ctrl+V` in your target app.

## Build from source

### Prerequisites

```bash
# Runtime
sudo dnf install wl-clipboard

# Build toolchain
sudo dnf install gtk3-devel webkit2gtk4.1-devel librsvg2-devel @development-tools

# Rust + Bun
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
curl -fsSL https://bun.sh/install | bash
```

### Build

```bash
git clone https://github.com/BraveRam/clipboard.git
cd clipboard
bun install
NO_STRIP=1 bun run tauri build --bundles appimage
```

> `NO_STRIP=1` is needed on Fedora 40+ because the `strip` bundled inside `linuxdeploy` doesn't recognize the newer `SHT_RELR` relocation type used by recent toolchains. The flag tells `linuxdeploy` to skip the strip pass.

Output: `src-tauri/target/release/bundle/appimage/Clipboard_*.AppImage`.

For day-to-day development:

```bash
bun run tauri dev          # hot-reload frontend + auto-rebuild Rust on change
cargo test --manifest-path src-tauri/Cargo.toml     # repository tests
```

## Architecture

```
                         OS-level keyboard shortcut
                                   │
                                   ▼
                  ┌────────────────────────────────────┐
                  │   tauri-plugin-single-instance      │
                  │   first launch  → start daemon      │
                  │   later launches → toggle window    │
                  └─────────────┬──────────────────────┘
                                │
              ┌─────────────────┴─────────────────┐
              ▼                                   ▼
   ┌──────────────────────┐         ┌──────────────────────────┐
   │ Rust core (Tauri 2)  │ events  │ React overlay (webview)  │
   │ ├─ arboard watcher   ├────────►│ ├─ SearchBar              │
   │ │   thread, 500 ms   │         │ ├─ EntryList              │
   │ ├─ rusqlite repo     │◄────────┤ └─ Keyboard navigation    │
   │ │   cap 50 unpinned  │ invoke  │                           │
   │ ├─ image thumbs      │         │                           │
   │ └─ Tauri commands    │         │                           │
   └──────────┬───────────┘         └──────────────────────────┘
              │
              ▼
   ~/.config/com.plxor.clipboard/
   ├── clipboard.db        SQLite (WAL)
   └── images/{sha256}.png Full-resolution captures
```

A polling thread in Rust uses [`arboard`](https://github.com/1Password/arboard) to read the system clipboard every 500 ms. Each change is hashed (SHA-256), deduplicated against the existing DB row, and persisted. Images are also written to disk as PNG and a 320-px-wide thumbnail is embedded as a BLOB for fast list rendering.

A `WriteGuard` records the hash of clipboard contents whenever the app *writes* (paste-back action), so the very next poll cycle doesn't re-capture our own write as a new entry.

## Configuration

Currently zero — the app is intentionally opinionated. The cap (`HISTORY_CAP = 50`), poll interval, hotkey for pin/delete, and window size are constants in `src-tauri/src/db.rs`, `src-tauri/src/clipboard.rs`, `src/hooks/useKeyboardNav.ts`, and `src-tauri/tauri.conf.json` respectively. PRs welcome to surface these in a settings panel.

### Data location

`~/.config/com.plxor.clipboard/`

To reset history:

```bash
rm -r ~/.config/com.plxor.clipboard
```

## Contributing

Issues and PRs welcome. A few ground rules:

- Keep dependencies lean — this is a daemon, not a framework.
- Tests for any new repository logic (see `src-tauri/src/db.rs#tests`).
- `cargo fmt` + `cargo clippy --all-targets -- -D warnings` before pushing.
- Frontend: `bun run build` must pass clean.

Ideas in the air, not yet implemented:

- Auto-paste (simulating `Ctrl+V` after `↵`) — needs uinput / portal permissions.
- HTML / RTF preservation.
- At-rest encryption.
- Wayland clipboard watch via `wl-paste --watch` as a more efficient fallback than polling.
- Configurable history cap + retention policy.

## Why another clipboard manager?

Linux clipboard managers either:

1. Live as a GNOME Shell extension that breaks every release, or
2. Are GTK 2 / Qt 4 antiques styled like Windows XP, or
3. Cost money and call home.

This one is a single AppImage that you drop in `~/Applications/`, bind to a key, and forget.

## License

[MIT](LICENSE)
