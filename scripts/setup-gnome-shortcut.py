#!/usr/bin/env python3
"""Install the GNOME launcher, or send a bounded toggle request with --toggle."""
import argparse
import ast
import datetime
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

BUS_NAME = 'com.plxor.clipboard.SingleInstance'
BUS_PATH = '/com/plxor/clipboard/SingleInstance'
DEBOUNCE_SECONDS = 0.25
IPC_TIMEOUT_SECONDS = 1.25
STARTUP_TIMEOUT_SECONDS = 8


def bus_call(destination, path, method, *arguments):
    return subprocess.run(
        ['gdbus', 'call', '--session', '--timeout', '1', '--dest', destination,
         '--object-path', path, '--method', method, *arguments],
        check=True, capture_output=True, text=True, timeout=IPC_TIMEOUT_SECONDS,
    ).stdout.strip()


def daemon_running():
    result = bus_call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                      'org.freedesktop.DBus.NameHasOwner', BUS_NAME)
    if result not in ('(true,)', '(false,)'):
        raise RuntimeError('Unexpected D-Bus ownership response')
    return result == '(true,)'


def process_identity(pid):
    """Distinguish a pending AppImage startup from a later reuse of its PID."""
    try:
        fields = Path(f'/proc/{int(pid)}/stat').read_text().rsplit(')', 1)[1].split()
        if fields[0] == 'Z':
            return None
        return fields[19]  # Linux /proc starttime (field 22).
    except (OSError, ValueError, IndexError, TypeError):
        return None


def save_state(lock, state):
    lock.seek(0)
    lock.truncate()
    json.dump(state, lock)
    lock.flush()


def toggle(appimage):
    """One IPC request in flight; never mount another AppImage for a busy daemon."""
    runtime = Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}'))
    lock_fd = os.open(runtime / 'clipboard-toggle.lock',
                      os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock_fd, 'r+') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return 0
        try:
            state = json.load(lock)
            if not isinstance(state, dict):
                state = {}
        except (ValueError, OSError):
            state = {}
        now = time.monotonic()
        last = state.get('last_request', 0)
        if isinstance(last, (float, int)) and 0 <= now - last < DEBOUNCE_SECONDS:
            return 0
        state['last_request'] = now
        save_state(lock, state)

        # Errors and timeouts propagate: only a confirmed absent name permits startup.
        if daemon_running():
            state.pop('starting_pid', None)
            state.pop('starting_identity', None)
            save_state(lock, state)
            token = os.environ.get('XDG_ACTIVATION_TOKEN') or os.environ.get('DESKTOP_STARTUP_ID')
            arguments = ['clipboard']
            if token:
                arguments.append(f'--xdg-token={token}')
            bus_call(BUS_NAME, BUS_PATH, 'org.SingleInstance.DBus.ExecuteCallback',
                     json.dumps(arguments, ensure_ascii=False),
                     json.dumps(os.getcwd(), ensure_ascii=False))
            return 0

        pid = state.get('starting_pid')
        identity = state.get('starting_identity')
        if identity and process_identity(pid) == identity:
            return 0
        child = subprocess.Popen(
            [str(appimage)], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, start_new_session=True,
        )
        state.update(starting_pid=child.pid, starting_identity=process_identity(child.pid))
        save_state(lock, state)
        deadline = time.monotonic() + STARTUP_TIMEOUT_SECONDS
        while time.monotonic() < deadline:
            if daemon_running():
                return 0  # Normal startup already presents with the inherited token.
            if child.poll() is not None:
                raise RuntimeError('Clipboard exited before registering on D-Bus')
            time.sleep(0.1)
        raise RuntimeError('Clipboard is still starting; another instance will not be launched')


def quote_exec(value):
    value = str(value)
    for old, new in [('\\', '\\\\'), ('"', '\\"'), ('`', '\\`'), ('$', '\\$'), ('%', '%%')]:
        value = value.replace(old, new)
    return f'"{value}"'


def install(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--appimage', type=Path, default=Path.home() / 'Applications/Clipboard.AppImage')
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args(argv)
    app = args.appimage.expanduser().resolve()
    if not app.is_file() or not os.access(app, os.X_OK):
        parser.error(f'AppImage must exist and be executable: {app}')
    for executable in ('gtk-launch', 'gdbus'):
        if not shutil.which(executable):
            parser.error(f'{executable} is required (install GTK3/GLib tools)')

    def get(schema, key):
        return ast.literal_eval(subprocess.check_output(['gsettings', 'get', schema, key], text=True).strip())

    schema = 'org.gnome.settings-daemon.plugins.media-keys'
    matches = []
    for path in get(schema, 'custom-keybindings'):
        binding_schema = schema + '.custom-keybinding:' + path
        if get(binding_schema, 'binding').lower() in ('<super>v', '<mod4>v'):
            matches.append(binding_schema)
    if len(matches) != 1:
        parser.error('Expected exactly one existing Super+V custom binding; no settings changed')
    binding_schema = matches[0]
    old_command = get(binding_schema, 'command')
    command = 'gtk-launch com.plxor.clipboard'
    data = Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share')))
    launcher = data / 'applications/com.plxor.clipboard.desktop'
    helper = data / 'clipboard/clipboard-toggle.py'
    source = Path(__file__).read_bytes()
    contents = f'''[Desktop Entry]
Type=Application
Name=Clipboard
Exec={quote_exec(sys.executable)} {quote_exec(helper)} --toggle {quote_exec(app)}
Icon=edit-paste
Terminal=false
StartupNotify=true
Categories=Utility;
'''
    if args.dry_run:
        print(json.dumps({'launcher': str(launcher), 'helper': str(helper),
                          'previous_command': old_command, 'command': command,
                          'desktop_entry': contents}, indent=2))
        return 0

    backup = data / 'clipboard/setup-backups' / datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
    backup.mkdir(parents=True)
    previous = {path: path.read_bytes() if path.exists() else None for path in (launcher, helper)}
    (backup / 'shortcut.json').write_text(json.dumps({
        'schema': binding_schema, 'command': old_command, 'launcher': str(launcher),
        'launcher_existed': previous[launcher] is not None,
        'helper': str(helper), 'helper_existed': previous[helper] is not None,
    }, indent=2))
    for path, value in previous.items():
        if value is not None:
            (backup / path.name).write_bytes(value)
    try:
        helper.parent.mkdir(parents=True, exist_ok=True)
        helper.write_bytes(source)
        launcher.parent.mkdir(parents=True, exist_ok=True)
        launcher.write_text(contents)
        subprocess.run(['gsettings', 'set', binding_schema, 'command', command], check=True)
        if get(binding_schema, 'command') != command:
            raise RuntimeError('Shortcut command did not persist')
    except Exception:
        for path, value in previous.items():
            if value is None:
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(value)
        subprocess.run(['gsettings', 'set', binding_schema, 'command', old_command], check=True)
        raise
    print(f'Installed {launcher}\nHelper: {helper}\nSuper+V: {command}\nBackup: {backup}')
    return 0


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) == 2 and argv[0] == '--toggle':
        try:
            return toggle(Path(argv[1]))
        except (OSError, subprocess.SubprocessError, RuntimeError) as error:
            # Do not expose activation token values from subprocess arguments/errors.
            print(f'Clipboard toggle failed ({type(error).__name__}); request stopped', file=sys.stderr)
            return 1
    return install(argv)


if __name__ == '__main__':
    sys.exit(main())
