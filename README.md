# Clipboard

> A fast, keyboard-driven clipboard history for Linux.

A small standalone desktop app that quietly tracks everything you copy (text and images) and lets you search and paste your history through a dark overlay summoned by a custom keyboard shortcut.

Built with [Tauri 2](https://tauri.app), React 19, and SQLite. Runs as a single-instance daemon. Bind any key combination in your desktop keyboard settings; pressing it toggles the existing window with instant search focus instead of spawning duplicate processes.

## Features

- **Text and image capture**: Automatic capture, deduplicated by SHA-256.
- **50-item rolling history**: Pinned items are stored separately and never evicted.
- **Fuzzy search**: Instant search across the whole history as you type.
- **Keyboard-first**: Arrow keys to navigate, Enter to paste back, Ctrl+P to pin, Ctrl+Backspace to delete, Esc to dismiss.
- **Vicinae theme and typography**: Styled with the Vicinae dark color palette and bundled Outfit and Geist Mono fonts.
- **Lucide vector icons**: Clean, minimal iconography throughout the interface.
- **Wayland native activation**: Forwards XDG activation tokens over D-Bus to prevent focus-stealing notifications and ensure instant keyboard input focus.
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
| GNOME    | Settings -> Keyboard -> Custom Shortcuts -> `+`; Command: `~/Applications/Clipboard.AppImage` |
| KDE      | System Settings -> Shortcuts -> Add Custom Shortcut -> Command/URL                           |
| Sway/i3  | `bindsym $mod+v exec ~/Applications/Clipboard.AppImage`                                      |
| Hyprland | `bind = SUPER, V, exec, ~/Applications/Clipboard.AppImage`                                   |

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
| `Enter`          | Paste and hide    |
| `Ctrl+P`         | Toggle pin        |
| `Ctrl+Backspace` | Delete entry      |
| `Esc`            | Hide overlay      |

After pressing Enter, the selected entry is placed onto your system clipboard and pasted into your active application.

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
- **Wayland Window Activation**: The primary daemon consumes the forwarded XDG activation token and presents the window using GTK `xdg_activation_v1` protocols, preventing focus-stealing notifications and ensuring instant keyboard focus.
- **React Frontend**: Rendered inside a WebKitGTK webview, styled with Vicinae dark tokens and variable fonts (`Outfit` and `Geist Mono`), communicating with Rust through Tauri IPC commands and events.
- **Write Guard**: Prevents re-capturing self-written clipboard content when pasting entries back to target applications.

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

## License

[MIT](LICENSE)
