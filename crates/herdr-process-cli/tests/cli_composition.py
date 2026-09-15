import contextlib
import fcntl
import json
import os
from pathlib import Path
import select
import socket
import struct
import subprocess
import sys
import termios
import time
import unittest

BINARY = os.path.abspath(sys.argv.pop(1))
# Deliberately /tmp, not TMPDIR: these roots hold Unix sockets, whose
# absolute path must fit SUN_LEN, and the per-user temporary directory on
# macOS already spends most of that budget (mirrors the Rust TempRoot).
ROOT = Path('/tmp')


class Composition(unittest.TestCase):
    def setUp(self):
        self.start = time.monotonic()
        import tempfile
        self.root = Path(tempfile.mkdtemp(prefix='cli-', dir=ROOT))
        self.env = {'HOME': str(self.root), 'PATH': '/usr/bin:/bin', 'TMPDIR': str(self.root)}
        self.children = []
        self.resources = contextlib.ExitStack()

    def tearDown(self):
        for child in self.children:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=.2)
            for pipe in [child.stdin, child.stdout, child.stderr]:
                if pipe is not None:
                    pipe.close()
        self.resources.close()
        elapsed = time.monotonic() - self.start
        print(f'{self.id()}: {elapsed * 1000:.1f} ms', file=sys.stderr)
        self.assertLess(elapsed, 1)

    def spawn(self, args, **kwargs):
        child = subprocess.Popen([BINARY, *args], env=self.env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
        self.children.append(child)
        return child

    def run_cli(self, args):
        child = self.spawn(args)
        out, err = child.communicate(timeout=.6)
        return child.returncode, out, err

    def config(self):
        herdr = self.root / 'config.toml'
        profiles = self.root / 'processes.toml'
        herdr.write_text('[keys]\nprefix="ctrl+a"\n')
        profiles.write_text('[windows.test]\nprogram="/bin/cat"\ncwd="~/"\n'
                            'width=80\nheight=70\nctrl_c="hide"\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr)
        return herdr, profiles

    def test_help_does_not_open_config(self):
        marker = self.root / 'blocked'
        os.mkfifo(marker)
        self.env['HERDR_CONFIG_PATH'] = str(marker)
        code, out, err = self.run_cli(['--help'])
        self.assertEqual(code, 0, err)
        self.assertIn(b'generate --output DIRECTORY', out)
        self.assertIn(b'action PROFILE split-right|split-below|toggle-float|kill', out)
        self.assertNotIn(b'supervise', out)

    def test_invalid_arguments_and_invalid_utf8_exit_two(self):
        marker = self.root / 'blocked'
        os.mkfifo(marker)
        self.env['HERDR_CONFIG_PATH'] = str(marker)
        for args in [['generate', '--wrong', 'x'], ['attach', '--profiles', 'x'], [b'\xff']]:
            code, _, err = self.run_cli(args)
            self.assertEqual(code, 2)
            self.assertTrue(err)

    def test_generate_uses_environment_selected_config(self):
        self.config()
        output = self.root / 'generated'
        output.mkdir()
        code, _, err = self.run_cli(['generate', '--output', str(output)])
        self.assertEqual(code, 0, err)
        self.assertIn('test:split-right', (output / 'herdr-plugin.toml').read_text())

    def test_runtime_errors_are_one_and_home_is_required_absolute(self):
        for home in [None, 'relative']:
            if home is None:
                self.env.pop('HOME', None)
            else:
                self.env['HOME'] = home
            code, _, err = self.run_cli(['generate', '--output', str(self.root / 'output')])
            self.assertEqual(code, 1)
            self.assertIn(b'HOME', err)

    def test_action_rejects_invalid_profile_before_start(self):
        self.config()
        self.env.update(HERDR_ENV='1', HERDR_BIN_PATH='/invalid/host',
                        HERDR_SOCKET_PATH=str(self.root / 'host.sock'))
        code, _, err = self.run_cli(['action', 'absent', 'kill'])
        self.assertEqual(code, 1)
        self.assertIn(b'unknown profile', err)

    def test_manager_lock_duplicate_and_private_lifetime(self):
        self.config()
        self.env.update(HERDR_BIN_PATH='/invalid/private-host',
                        HERDR_SOCKET_PATH=str(self.root / 'host.sock'), HERDR_PROCESS_STARTUP='1')
        runtime = self.root / 'runtime'
        first = self.spawn(['manager', '--runtime-dir', str(runtime)])
        self.assertTrue(select.select([first.stderr], [], [], .25)[0], 'manager readiness missing')
        self.assertEqual(os.read(first.stderr.fileno(), 256), b'herdr-process:ready\n')
        code, out, err = self.run_cli(['manager', '--runtime-dir', str(runtime)])
        self.assertEqual((code, out, err), (0, b'', b'herdr-process:duplicate\n'))
        self.assertIsNone(first.poll())

    def test_duplicate_manager_does_not_reload_configuration(self):
        self.config()
        self.env.update(HERDR_BIN_PATH='/invalid/private-host',
                        HERDR_SOCKET_PATH=str(self.root / 'host.sock'), HERDR_PROCESS_STARTUP='1')
        runtime = self.root / 'runtime'
        first = self.spawn(['manager', '--runtime-dir', str(runtime)])
        self.assertTrue(select.select([first.stderr], [], [], .25)[0])
        self.assertEqual(os.read(first.stderr.fileno(), 256), b'herdr-process:ready\n')
        marker = self.root / 'blocked-profiles'
        os.mkfifo(marker)
        code, out, err = self.run_cli(['manager', '--runtime-dir', str(runtime), '--profiles', str(marker)])
        self.assertEqual((code, out, err), (0, b'', b'herdr-process:duplicate\n'))
        self.assertIsNone(first.poll())

    def test_action_requires_host_context_and_split_target(self):
        self.config()
        self.env.update(HERDR_BIN_PATH='/invalid/private-host',
                        HERDR_SOCKET_PATH=str(self.root / 'host.sock'))
        code, _, err = self.run_cli(['action', 'test', 'kill'])
        self.assertEqual(code, 1)
        self.assertIn(b'HERDR_ENV=1', err)
        self.env['HERDR_ENV'] = '1'
        code, _, err = self.run_cli(['action', 'test', 'split-right'])
        self.assertEqual(code, 1)
        self.assertIn(b'target pane', err)

    def test_invalid_attachment_ticket_is_never_printed(self):
        self.env.update(HERDR_PROCESS_SOCKET=str(self.root / 'session.sock'),
                        HERDR_PROCESS_PROFILE='test', HERDR_PROCESS_TICKET=b'private-secret-\xff')
        code, _, err = self.run_cli(['attach'])
        self.assertEqual(code, 1)
        self.assertNotIn(b'private-secret', err)

    def test_supervise_dispatch_does_not_load_configuration(self):
        marker = self.root / 'blocked'
        os.mkfifo(marker)
        self.env['HERDR_CONFIG_PATH'] = str(marker)
        self.env.pop('HOME')
        code, _, err = self.run_cli(['supervise', str(self.root / 'absent.sock'), 'private-ticket'])
        self.assertEqual(code, 1)
        self.assertNotIn(b'HOME', err)
        self.assertNotIn(b'private-ticket', err)

    def frame(self, connection, message):
        payload = json.dumps({'version': 1, 'message': message}, separators=(',', ':')).encode()
        connection.sendall(struct.pack('>I', len(payload)) + payload)

    def receive(self, connection):
        def exact(size):
            result = b''
            while len(result) < size:
                chunk = connection.recv(size - len(result))
                self.assertTrue(chunk, 'unexpected EOF')
                result += chunk
            return result
        count, = struct.unpack('>I', exact(4))
        return json.loads(exact(count))['message']

    def attachment(self):
        path = self.root / 'session.sock'
        server = self.resources.enter_context(socket.socket(socket.AF_UNIX))
        server.bind(str(path))
        server.listen(1)
        server.settimeout(.3)
        master, slave = os.openpty()
        # Close the master first; an unread slave output queue must not delay teardown.
        self.resources.callback(os.close, slave)
        self.resources.callback(os.close, master)
        original = termios.tcgetattr(slave)
        flags = fcntl.fcntl(slave, fcntl.F_GETFL)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
        self.env.update(HERDR_PROCESS_SOCKET=str(path), HERDR_PROCESS_PROFILE='test',
                        HERDR_PROCESS_TICKET='private-ticket')
        child = subprocess.Popen([BINARY, 'attach'], env=self.env, stdin=slave, stdout=slave,
                                 stderr=subprocess.PIPE)
        self.children.append(child)
        connection, _ = server.accept()
        self.resources.enter_context(connection)
        connection.settimeout(.3)
        self.assertEqual(self.receive(connection), {'type': 'attach', 'profile': 'test',
                         'ticket': 'private-ticket', 'rows': 24, 'cols': 80})
        return child, connection, master, slave, original, flags

    def assert_restored(self, slave, original, flags):
        actual = termios.tcgetattr(slave)
        # Darwin reports transient pending canonical input after restoration.
        actual[3] &= ~termios.PENDIN
        original[3] &= ~termios.PENDIN
        self.assertEqual(actual, original)
        restorable = os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC | os.O_SYNC | os.O_ACCMODE
        self.assertEqual(fcntl.fcntl(slave, fcntl.F_GETFL) & restorable, flags & restorable)

    def read_terminal(self, master, suffix):
        output = b''
        deadline = time.monotonic() + .3
        while suffix not in output:
            self.assertLess(time.monotonic(), deadline, repr(output[-100:]))
            if select.select([master], [], [], .01)[0]:
                output += os.read(master, 65536)
        return output

    def test_attachment_replay_ack_raw_input_resize_and_retire(self):
        child, connection, master, slave, original, flags = self.attachment()
        raw = b'unfinished\x03\x1b[200~pasted\x03\x1b[201~\x1b['
        os.write(master, raw)
        self.assertFalse(select.select([connection], [], [], .015)[0])
        screen = b'\x1b[2J\x1b[Hprivate-screen'
        self.frame(connection, {'type': 'screen', 'bytes': list(screen)})
        self.frame(connection, {'type': 'attached'})
        self.assertEqual(self.receive(connection), {'type': 'ready'})
        self.assertIn(screen, self.read_terminal(master, b'private-screen'))
        self.assertFalse(select.select([connection], [], [], .015)[0])
        self.frame(connection, {'type': 'ack'})
        forwarded = b''
        while len(forwarded) < len(raw):
            message = self.receive(connection)
            self.assertEqual(message['type'], 'input')
            forwarded += bytes(message['bytes'])
        self.assertEqual(forwarded, raw)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 35, 101, 0, 0))
        self.assertEqual(self.receive(connection), {'type': 'resize', 'rows': 35, 'cols': 101})
        self.frame(connection, {'type': 'retire'})
        self.assertEqual(self.receive(connection), {'type': 'detach'})
        self.assertEqual(child.wait(timeout=.3), 0, child.stderr.read())
        self.assert_restored(slave, original, flags)
        self.assertIn(b'\x1b[?1049l', self.read_terminal(master, b'\x1b[?1049l'))

    def test_attachment_backpressure_keeps_every_partial_screen_byte(self):
        child, connection, master, slave, original, flags = self.attachment()
        screen = b'\x1b[31m' + b'0123456789' * 3000 + b'\x1b[0mSCREEN-END'
        self.frame(connection, {'type': 'screen', 'bytes': list(screen)})
        self.frame(connection, {'type': 'attached'})
        self.assertFalse(select.select([connection], [], [], .015)[0])
        output = self.read_terminal(master, b'SCREEN-END')
        self.assertEqual(output, b'\x1b[?1049h' + screen)
        self.assertEqual(self.receive(connection), {'type': 'ready'})
        self.frame(connection, {'type': 'ack'})
        self.frame(connection, {'type': 'retire'})
        self.assertEqual(self.receive(connection), {'type': 'detach'})
        self.assertEqual(child.wait(timeout=.3), 0)
        self.assert_restored(slave, original, flags)

    def test_retire_while_terminal_blocked_restores_without_hanging(self):
        child, connection, master, slave, original, flags = self.attachment()
        self.frame(connection, {'type': 'screen', 'bytes': [65] * 30000})
        self.frame(connection, {'type': 'attached'})
        self.frame(connection, {'type': 'retire'})
        self.assertEqual(self.receive(connection), {'type': 'detach'})
        self.assertIn(child.wait(timeout=.3), [0, 1])
        self.assert_restored(slave, original, flags)

    def test_attachment_unwind_restores_terminal(self):
        child, connection, master, slave, original, flags = self.attachment()
        self.frame(connection, {'type': 'error', 'message': 'private unwind fixture'})
        expected = 101 if os.environ.get('CLI_EXPECT_PANIC') == '1' else 1
        self.assertEqual(child.wait(timeout=.3), expected)
        self.assert_restored(slave, original, flags)
        self.assertIn(b'\x1b[?1049l', self.read_terminal(master, b'\x1b[?1049l'))

    def test_attachment_disconnect_restores(self):
        child, connection, master, slave, original, flags = self.attachment()
        connection.close()
        self.assertEqual(child.wait(timeout=.3), 1)
        self.assert_restored(slave, original, flags)

    def test_attachment_handshake_error_preserves_message(self):
        child, connection, master, slave, original, flags = self.attachment()
        self.frame(connection, {'type': 'error', 'message': 'stale attachment ticket'})
        self.assertEqual(child.wait(timeout=.3), 1)
        self.assertIn(b'stale attachment ticket', child.stderr.read())
        self.assert_restored(slave, original, flags)


if __name__ == '__main__':
    unittest.main(verbosity=2)
