# Clipboard

> A fast, keyboard-driven clipboard history for Linux.

A small standalone desktop app that quietly tracks everything you copy (text and images) and lets you search and copy from your history through a dark overlay summoned by a custom keyboard shortcut.

Built with [Tauri 2](https://tauri.app), React 19, and SQLite. Runs as a single-instance daemon. Bind any key combination in your desktop keyboard settings; pressing it toggles the existing window instead of spawning duplicate processes.

## Features

- **Text and image capture**: Automatic capture, deduplicated by SHA-256.
- **50-item rolling history**: Pinned items are exempt from eviction. Unpinning restores the original recency, so an old item can be evicted immediately.
- **Fuzzy search**: Instant search across the whole history as you type.
- **Keyboard-first**: Arrow keys to navigate, Enter to copy and close, Ctrl+P to pin, Ctrl+Backspace to delete, Esc to dismiss.
- **Vicinae theme and typography**: Styled with the Vicinae dark color palette and bundled Outfit and Geist Mono fonts.
- **Lucide vector icons**: Clean, minimal iconography throughout the interface.
- **Wayland native activation**: Forwards available XDG activation tokens over D-Bus; keyboard focus is subject to compositor policy.
- **Privacy-respecting**: Everything stays locally in `~/.config/com.plxor.clipboard/`. No network calls, ever.
- **Single binary**: Self-contained AppImage bundling GTK and WebKit for modern Linux distributions.

## Install

Grab the AppImage from the [latest release](../../releases/latest):

```bash
mkdir -p ~/Applications
mv ~/Downloads/Clipboard_*.AppImage ~/Applications/Clipboard.AppImage
chmod +x ~/Applications/Clipboard.AppImage
```

Then bind it to a keyboard shortcut:

| Desktop  | Configuration                                                                                |
| -------- | -------------------------------------------------------------------------------------------- |
| GNOME    | Settings -> Keyboard -> Custom Shortcuts -> `+`; Command: `gtk-launch com.plxor.clipboard` |
| KDE      | System Settings -> Shortcuts -> Add Custom Shortcut -> Command/URL                           |
| Sway/i3  | `bindsym $mod+v exec ~/Applications/Clipboard.AppImage`                                      |
| Hyprland | `bind = SUPER, V, exec, ~/Applications/Clipboard.AppImage`                                   |

On GNOME, first create a Super+V custom shortcut, then install the desktop launcher. Download the setup script from the same release as the AppImage:

```bash
curl -fL -o ~/Downloads/setup-gnome-shortcut.py https://github.com/BraveRam/clipboard/releases/latest/download/setup-gnome-shortcut.py
python3 ~/Downloads/setup-gnome-shortcut.py --appimage ~/Applications/Clipboard.AppImage
```

When building from source, run `python3 scripts/setup-gnome-shortcut.py` instead.

The setup command requires `gtk-launch`, backs up the existing shortcut command and launcher, and updates the existing Super+V binding. Use `--dry-run` to preview changes. It does not create duplicate bindings. The launcher enables startup notification so the app can receive a focus activation token; launching the AppImage directly from a GNOME custom shortcut omits that token. Other desktop environments require their own activation-aware launch setup.

The first press of the shortcut launches the daemon and displays the overlay. Subsequent presses toggle visibility on the running daemon.

### Start at login

On its first run, the app enables launch-at-login automatically so clipboard history continues capturing across sessions. At login, it starts minimized in the system tray.

The system tray menu provides:

- Show clipboard: open the overlay
- Start at login: toggle autostart on or off
- Quit: stop the daemon

Launch-at-login uses an XDG autostart entry (`~/.config/autostart/Clipboard.desktop`), supported out of the box by GNOME, KDE, XFCE, and LXQt.

For tiling window managers (i3, Sway, Hyprland):

| Window Manager | Configuration Command                                       |
| -------------- | ----------------------------------------------------------- |
| Sway/i3        | `exec ~/Applications/Clipboard.AppImage --minimized`        |
| Hyprland       | `exec-once = ~/Applications/Clipboard.AppImage --minimized` |

### Wayland users

Install `wl-clipboard` so the clipboard watcher can read Wayland selections:

```bash
sudo dnf install wl-clipboard       # Fedora
sudo apt install wl-clipboard       # Debian / Ubuntu
sudo pacman -S wl-clipboard         # Arch
```

## Usage

| Key              | Action            |
| ---------------- | ----------------- |
| `Up` / `Down`    | Navigate entries  |
| `Enter`          | Copy and hide    |
| `Ctrl+P`         | Toggle pin        |
| `Ctrl+Backspace` | Delete entry      |
| `Esc`            | Hide overlay      |

After pressing Enter, the selected entry is placed onto your system clipboard and the overlay closes. Paste it yourself in the destination application.

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

Output: `src-tauri/target/release/bundle/appimage/Clipboard_*.AppImage`.

Development commands:

```bash
bun run tauri dev                                  # hot-reload frontend + auto-rebuild backend
cargo test --manifest-path src-tauri/Cargo.toml     # run backend test suite
```

## Architecture

- **Clipboard Watcher**: An `arboard` background polling thread running at 500 ms intervals reads the system clipboard across Wayland and X11 sessions. Each entry is hashed with SHA-256 and deduplicated. Text entries are saved directly in SQLite; images are saved to disk with embedded thumbnail previews.
- **Single Instance Daemon**: When invoked via keyboard shortcut, a new process checks for an existing session via D-Bus (`com.plxor.clipboard.SingleInstance`). It forwards the environment activation token (`XDG_ACTIVATION_TOKEN`) and command-line arguments to the running daemon before immediately exiting.
- **Wayland Window Activation**: The primary daemon supplies the forwarded XDG activation token to GTK before presenting the window. The compositor decides whether to grant focus.
- **React Frontend**: Rendered inside a WebKitGTK webview, styled with Vicinae dark tokens and variable fonts (`Outfit` and `Geist Mono`), communicating with Rust through Tauri IPC commands and events.
- **Write Guard**: Prevents re-capturing self-written clipboard content when copying history entries back to the clipboard.

## Configuration

Settings are currently hardcoded for optimal latency and workflow:
- History capacity: 50 unpinned entries (pinned entries are unbounded)
- Poll interval: 500 ms
- Window size: 770 x 480 px

Data storage path: `~/.config/com.plxor.clipboard/` (SQLite database at `clipboard.db` and full images at `images/`).

To reset history:

```bash
rm -r ~/.config/com.plxor.clipboard
```

## Contributing

- Keep dependencies lean.
- Write tests for repository and database logic.
- Ensure `cargo test` and `bun run build` pass before submitting PRs.

## Focus diagnostics

Set `CLIPBOARD_DEBUG_FOCUS=1` when starting the daemon to log activation-token presence and native focus transitions. Logs never include token values or clipboard contents. Focus still depends on the desktop accepting the launch token; repeatedly focusing the search input cannot override compositor policy.

**GNOME/Wayland validation:** With the desktop launcher and the updated AppImage, Super+V focused search from another open app and a second press hid the overlay on GNOME 50. Other compositors may handle activation differently.

## License

[MIT](LICENSE)
