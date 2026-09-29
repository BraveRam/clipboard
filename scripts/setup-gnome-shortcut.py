#!/usr/bin/env python3
"""Install the Clipboard launcher and update the existing GNOME Super+V binding."""
import argparse
import ast
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--appimage', type=Path, default=Path.home() / 'Applications/Clipboard.AppImage')
parser.add_argument('--dry-run', action='store_true')
args = parser.parse_args()
app = args.appimage.expanduser().resolve()
if not app.is_file() or not os.access(app, os.X_OK):
    parser.error(f'AppImage must exist and be executable: {app}')
if not shutil.which('gtk-launch'):
    parser.error('gtk-launch is required (install the gtk3 package)')

def get(schema, key):
    return ast.literal_eval(subprocess.check_output(['gsettings', 'get', schema, key], text=True).strip())

schema = 'org.gnome.settings-daemon.plugins.media-keys'
paths = get(schema, 'custom-keybindings')
matches = []
for path in paths:
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
# Desktop Entry Exec escaping (not shell quoting).
quoted = str(app).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
contents = f'''[Desktop Entry]
Type=Application
Name=Clipboard
Exec="{quoted}"
Icon=edit-paste
Terminal=false
StartupNotify=true
Categories=Utility;
'''
if args.dry_run:
    print(json.dumps({'launcher': str(launcher), 'previous_command': old_command, 'command': command, 'desktop_entry': contents}, indent=2))
else:
    backup = data / 'clipboard/setup-backups' / datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
    backup.mkdir(parents=True)
    (backup / 'shortcut.json').write_text(json.dumps({'schema': binding_schema, 'command': old_command, 'launcher': str(launcher), 'launcher_existed': launcher.exists()}, indent=2))
    previous = launcher.read_bytes() if launcher.exists() else None
    if previous is not None:
        (backup / launcher.name).write_bytes(previous)
    launcher.parent.mkdir(parents=True, exist_ok=True)
    try:
        launcher.write_text(contents)
        subprocess.run(['gsettings', 'set', binding_schema, 'command', command], check=True)
        if get(binding_schema, 'command') != command:
            raise RuntimeError('Shortcut command did not persist')
    except Exception:
        if previous is None:
            launcher.unlink(missing_ok=True)
        else:
            launcher.write_bytes(previous)
        subprocess.run(['gsettings', 'set', binding_schema, 'command', old_command], check=True)
        raise
    print(f'Installed {launcher}\nSuper+V: {command}\nBackup: {backup}')
