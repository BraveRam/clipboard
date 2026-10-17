"""Exercise the helper against an isolated D-Bus service, never the desktop."""
from concurrent.futures import ThreadPoolExecutor
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time

from gi.repository import Gio, GLib

source = Path(__file__).resolve().parents[1] / 'setup-gnome-shortcut.py'
xml = '''<node><interface name="org.SingleInstance.DBus">
<method name="ExecuteCallback"><arg type="as" direction="in"/>
<arg type="s" direction="in"/></method></interface></node>'''
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
requests = []
state = {'pending': 0, 'max_pending': 0, 'stall': False}


def callback(_connection, _sender, _path, _interface, _method, parameters, invocation):
    requests.append(parameters.unpack())
    if state['stall']:
        return
    state['pending'] += 1
    state['max_pending'] = max(state['max_pending'], state['pending'])

    def reply():
        state['pending'] -= 1
        invocation.return_value(GLib.Variant('()', ()))
        return False

    GLib.timeout_add(150, reply)


connection.register_object('/com/plxor/clipboard/SingleInstance',
                           Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0], callback, None, None)
connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                     'org.freedesktop.DBus', 'RequestName',
                     GLib.Variant('(su)', ('com.plxor.clipboard.SingleInstance', 0)),
                     None, Gio.DBusCallFlags.NONE, 1000, None)
loop = GLib.MainLoop()
thread = threading.Thread(target=loop.run, daemon=True)
thread.start()
try:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        helper = root / 'installed helper.py'
        shutil.copyfile(source, helper)
        app = root / 'fake AppImage'
        launches = root / 'launches'
        app.write_text(f'#!/bin/sh\nprintf launch >> "{launches}"\n')
        app.chmod(0o755)
        env = dict(os.environ, XDG_RUNTIME_DIR=directory,
                   XDG_ACTIVATION_TOKEN='', DESKTOP_STARTUP_ID='legacy-focus-token')

        def toggle(_=None):
            return subprocess.run([sys.executable, str(helper), '--toggle', str(app)],
                                  cwd=directory, env=env, capture_output=True, timeout=3)

        result = toggle()
        assert result.returncode == 0, result.stderr
        assert requests[0] == (['clipboard', '--xdg-token=legacy-focus-token'], directory)
        time.sleep(0.3)
        started = time.monotonic()
        with ThreadPoolExecutor(max_workers=15) as workers:
            results = list(workers.map(toggle, range(50)))
        elapsed = time.monotonic() - started
        assert all(result.returncode == 0 for result in results)
        assert state['max_pending'] == 1, state
        assert len(requests) <= math.ceil(elapsed / 0.25) + 2, (elapsed, len(requests))
        assert not launches.exists(), 'warm toggles launched an AppImage'
        time.sleep(0.3)
        state['stall'] = True
        started = time.monotonic()
        result = toggle()
        assert result.returncode == 1
        assert time.monotonic() - started < 2.5
        assert not launches.exists(), 'a stalled daemon caused a second startup'
        assert b'legacy-focus-token' not in result.stderr
        binary = os.environ.get('CLIPBOARD_TEST_BINARY')
        if binary:
            # No display is available to this child even if forwarding regresses.
            native_env = {key: value for key, value in env.items()
                          if key not in ('DISPLAY', 'WAYLAND_DISPLAY')}
            native_env['GDK_BACKEND'] = 'none'
            started = time.monotonic()
            result = subprocess.run([binary], env=native_env, capture_output=True, timeout=3)
            assert result.returncode == 0, result.stderr
            assert b'running instance did not respond' in result.stderr
            assert time.monotonic() - started < 2
            native_env['DBUS_SESSION_BUS_ADDRESS'] = f'unix:path={root}/missing-bus'
            result = subprocess.run([binary], env=native_env, capture_output=True, timeout=3)
            assert result.returncode == 0, result.stderr
            assert b'session bus unavailable' in result.stderr
        print(f'50 concurrent requests: {len(requests)} bounded calls; max in flight = 1; no AppImage starts; timeout bounded')
finally:
    loop.quit()
    thread.join(timeout=2)
