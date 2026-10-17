"""Regression tests for the installed helper; no real desktop is touched."""
import contextlib
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('shortcut', Path(__file__).with_name('setup-gnome-shortcut.py'))
shortcut = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shortcut)


class ToggleTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.runtime = Path(self.directory.name)
        self.environment = patch.dict(os.environ, {
            'XDG_RUNTIME_DIR': str(self.runtime), 'XDG_ACTIVATION_TOKEN': '',
            'DESKTOP_STARTUP_ID': 'legacy-token',
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.clock = patch.object(shortcut.time, 'monotonic', return_value=100)
        self.now = self.clock.start()
        self.addCleanup(self.clock.stop)

    def test_warm_toggle_never_launches_appimage_and_preserves_token(self):
        with patch.object(shortcut, 'daemon_running', return_value=True), \
             patch.object(shortcut, 'bus_call') as call, \
             patch.object(shortcut.subprocess, 'Popen') as launch:
            self.assertEqual(shortcut.toggle(Path('/unused.AppImage')), 0)
            self.assertEqual(json.loads(call.call_args.args[3]), ['clipboard', '--xdg-token=legacy-token'])
            launch.assert_not_called()

    def test_burst_of_100_requests_is_debounced(self):
        with patch.object(shortcut, 'daemon_running', return_value=True) as owner, \
             patch.object(shortcut, 'bus_call') as call:
            for _ in range(100):
                shortcut.toggle(Path('/unused.AppImage'))
            owner.assert_called_once()
            call.assert_called_once()
            self.now.return_value = 100.25
            shortcut.toggle(Path('/unused.AppImage'))
            self.assertEqual(call.call_count, 2)

    def test_overlapping_helper_exits_without_waiting_or_calling_dbus(self):
        with (self.runtime / 'clipboard-toggle.lock').open('w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(shortcut, 'daemon_running') as owner:
                self.assertEqual(shortcut.toggle(Path('/unused.AppImage')), 0)
                owner.assert_not_called()

    def test_busy_daemon_never_falls_back_to_startup(self):
        secret = 'do-not-log-this-token'
        with patch.object(shortcut, 'daemon_running', return_value=True), \
             patch.object(shortcut, 'bus_call', side_effect=subprocess.TimeoutExpired(secret, 1)), \
             patch.object(shortcut.subprocess, 'Popen') as launch, \
             contextlib.redirect_stderr(io.StringIO()) as log:
            self.assertEqual(shortcut.main(['--toggle', '/unused.AppImage']), 1)
            launch.assert_not_called()
            self.assertNotIn(secret, log.getvalue())

    def test_unavailable_bus_does_not_start_appimage(self):
        with patch.object(shortcut, 'daemon_running', side_effect=subprocess.CalledProcessError(1, 'gdbus')), \
             patch.object(shortcut.subprocess, 'Popen') as launch, \
             contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(shortcut.main(['--toggle', '/unused.AppImage']), 1)
            launch.assert_not_called()

    def test_cold_start_is_single_and_retains_startup_guard(self):
        child = Mock(pid=42)
        with patch.object(shortcut, 'daemon_running', side_effect=[False, True, False]), \
             patch.object(shortcut, 'process_identity', return_value='birth'), \
             patch.object(shortcut.subprocess, 'Popen', return_value=child) as launch:
            self.assertEqual(shortcut.toggle(Path('/test.AppImage')), 0)
            self.now.return_value = 101
            self.assertEqual(shortcut.toggle(Path('/test.AppImage')), 0)
            launch.assert_called_once()
            self.assertEqual(launch.call_args.args[0], ['/test.AppImage'])

    def test_startup_timeout_does_not_permit_another_live_startup(self):
        with patch.object(shortcut, 'daemon_running', return_value=False), \
             patch.object(shortcut, 'process_identity', return_value='birth'), \
             patch.object(shortcut.subprocess, 'Popen', return_value=Mock(pid=42)) as launch:
            self.now.side_effect = [100, 100, 109]
            with self.assertRaisesRegex(RuntimeError, 'still starting'):
                shortcut.toggle(Path('/test.AppImage'))
            self.now.side_effect = None
            self.now.return_value = 110
            self.assertEqual(shortcut.toggle(Path('/test.AppImage')), 0)
            launch.assert_called_once()

    def test_bus_process_has_short_timeout(self):
        with patch.object(shortcut.subprocess, 'run', return_value=Mock(stdout='(true,)')) as run:
            self.assertTrue(shortcut.daemon_running())
            self.assertEqual(run.call_args.kwargs['timeout'], 1.25)
            self.assertIn('--timeout', run.call_args.args[0])


class PrivateBusTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('dbus-run-session'), 'dbus-run-session is required')
    def test_real_bus_burst_and_timeout(self):
        python = '/usr/bin/python3'
        probe = subprocess.run([python, '-c', 'from gi.repository import Gio, GLib'], capture_output=True)
        if probe.returncode:
            self.skipTest('system Python GObject bindings are required for the fake service')
        fixture = Path(__file__).with_name('fixtures') / 'shortcut_bus.py'
        result = subprocess.run(['dbus-run-session', '--', python, str(fixture)],
                                capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('max in flight = 1', result.stdout)


class InstallTests(unittest.TestCase):
    def test_installer_is_self_contained_and_restores_previous_files_on_error(self):
        with tempfile.TemporaryDirectory() as directory:
            data = Path(directory)
            app = data / 'Clipboard.AppImage'
            app.write_text('#!/bin/sh\nexit 0\n')
            app.chmod(0o755)
            launcher = data / 'applications/com.plxor.clipboard.desktop'
            launcher.parent.mkdir()
            launcher.write_bytes(b'old launcher')
            helper = data / 'clipboard/clipboard-toggle.py'
            helper.parent.mkdir()
            helper.write_bytes(b'old helper')
            command = ['old command']

            def get(args, **kwargs):
                key = args[-1]
                return repr({'custom-keybindings': ['/custom/'], 'binding': '<Super>v',
                             'command': command[0]}[key])

            def set_command(args, **kwargs):
                command[0] = args[-1]
                return Mock()

            with patch.dict(os.environ, {'XDG_DATA_HOME': directory}), \
                 patch.object(shortcut.shutil, 'which', return_value='/usr/bin/tool'), \
                 patch.object(shortcut.subprocess, 'check_output', side_effect=get), \
                 patch.object(shortcut.subprocess, 'run', side_effect=set_command), \
                 contextlib.redirect_stdout(io.StringIO()):
                shortcut.install(['--appimage', str(app)])
                self.assertEqual(helper.read_bytes(), Path(shortcut.__file__).read_bytes())
                contents = launcher.read_text()
                self.assertIn('--toggle', contents)
                self.assertIn('StartupNotify=true', contents)
                self.assertEqual(command[0], 'gtk-launch com.plxor.clipboard')
                launcher.write_bytes(b'prior launcher')
                helper.write_bytes(b'prior helper')
                with patch.object(shortcut.subprocess, 'run', side_effect=[RuntimeError('failure'), Mock()]):
                    with self.assertRaisesRegex(RuntimeError, 'failure'):
                        shortcut.install(['--appimage', str(app)])
                self.assertEqual(launcher.read_bytes(), b'prior launcher')
                self.assertEqual(helper.read_bytes(), b'prior helper')


if __name__ == '__main__':
    unittest.main()
