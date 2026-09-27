# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Qualify the real launchd worker from an ordinary user account."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import pwd
import select
import shutil
import subprocess
import tarfile
import tempfile
import time
import uuid


class Service:
    """Own one disposable service, its files and its exact launchd label."""

    def __init__(self, root: Path, worker: Path, fixture: Path, compiler: Path) -> None:
        """Prepare trusted files without registering the service yet."""
        if os.geteuid() != 0:
            raise RuntimeError('run the qualification through sudo')
        self.root = root
        self.label = f'ai.dedalus.bsmr.qualification.{uuid.uuid4().hex}'
        self.uid = int(os.environ['SUDO_UID'])
        if self.uid == 0:
            raise RuntimeError('qualification needs an ordinary invoking account')
        self.gid = pwd.getpwuid(self.uid).pw_gid
        self.worker = root / 'worker'
        self.socket = root / 'control.sock'
        self.info = self.socket.with_suffix('.json')
        self.plist = root / 'service.plist'
        self.data = root / 'data'
        self.active = False
        root.chmod(0o755)
        self.data.mkdir(mode=0o700)
        seed = self.data / 'seed'
        seed.mkdir()
        shutil.copyfile(worker, self.worker)
        os.chown(self.worker, 0, 0)
        self.worker.chmod(0o555)
        self.command(arguments=[str(fixture), 'seed', str(seed), str(compiler)])
        lease = self.data / 'identity'
        lease.touch(mode=0o600, exist_ok=False)
        config = root / 'config.json'
        config.write_text(
            json.dumps(
                {
                    'client': self.uid,
                    'identity': 60_002,
                    'seed': str(seed),
                    'state': str(self.data),
                    'socket': str(self.socket),
                }
            )
        )
        config.chmod(0o600)
        description = {
            'Label': self.label,
            'ProgramArguments': [str(self.worker), 'serve', str(config)],
            'RunAtLoad': True,
            'StandardOutPath': str(self.data / 'stdout'),
            'StandardErrorPath': str(self.data / 'stderr'),
            'Sockets': {
                'Listener': {
                    'SockType': 'stream',
                    'SockPathName': str(self.socket),
                    'SockPathMode': 0o600,
                }
            },
        }
        self.plist.write_bytes(plistlib.dumps(description))

    @staticmethod
    def command(arguments: list[str]) -> str:
        """Run an exact command and preserve failures rather than guessing success."""
        result = subprocess.run(
            arguments, capture_output=True, text=True, timeout=60, check=False
        )
        if result.returncode != 0:
            raise RuntimeError(
                f'{arguments[0]} exited {result.returncode}: {result.stdout}{result.stderr}'
            )
        return result.stdout

    def start(self) -> None:
        """Register only this service and wait on directory events for its ready record."""
        self.active = True
        self.command(arguments=['launchctl', 'bootstrap', 'system', str(self.plist)])
        descriptor = os.open(self.root, os.O_RDONLY)
        queue = select.kqueue()
        event = select.kevent(
            descriptor,
            filter=select.KQ_FILTER_VNODE,
            flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
            fflags=select.KQ_NOTE_WRITE,
        )
        try:
            queue.control([event], 0, 0)
            deadline = time.monotonic() + 60
            while not self.info.exists():
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not queue.control(None, 1, remaining):
                    log = self.data / 'stderr'
                    detail = log.read_text() if log.exists() else 'no worker log'
                    raise TimeoutError(f'native service did not become ready: {detail}')
        finally:
            queue.close()
            os.close(descriptor)

    def request(self) -> Path:
        """Compile through the authenticated service using user-owned file capabilities."""
        environment = json.loads(self.info.read_text())['environment']
        if not isinstance(environment, str):
            raise TypeError('worker environment must be a string')
        directory = self.root / 'client'
        directory.mkdir(mode=0o700)
        os.chown(directory, self.uid, self.gid)
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode='w') as archive:
            source = b'pub fn answer() -> u64 { 42 }\n'
            entry = tarfile.TarInfo('input.rs')
            entry.mode = 0o644
            entry.size = len(source)
            archive.addfile(entry, io.BytesIO(source))
        payload = data.getvalue()
        action = {
            'environment': environment,
            'input': hashlib.sha256(payload).hexdigest(),
            'action': {
                'protocol': 1,
                'arguments': [
                    '/toolchain/bin/rustc',
                    '--crate-type=lib',
                    '--emit=metadata',
                    'input.rs',
                    '-o',
                    'result.rmeta',
                ],
                'environment': {},
                'working_directory': '',
                'outputs': [{'path': 'result.rmeta', 'kind': 'file'}],
                'timeout_ms': 10_000,
            },
        }
        for name, content in [
            ('action.json', json.dumps(action).encode()),
            ('input.tar', payload),
            ('output.tar', b''),
        ]:
            path = directory / name
            path.write_bytes(content)
            os.chown(path, self.uid, self.gid)
            path.chmod(0o600)
        return directory

    def qualify(self) -> None:
        """Compile through the authenticated service using user-owned file capabilities."""
        directory = self.request()
        result = self.command(
            arguments=[
                'sudo',
                '-n',
                '-u',
                f'#{self.uid}',
                '--',
                str(self.worker),
                'exchange',
                str(self.socket),
                str(directory / 'action.json'),
                str(directory / 'input.tar'),
                str(directory / 'output.tar'),
                '--timeout-seconds',
                '30',
            ]
        )
        assert json.loads(result) == 'completed', result
        with tarfile.open(directory / 'output.tar') as archive:
            stream = archive.extractfile('.bsmr/result.json')
            assert stream is not None
            envelope = json.load(stream)
            assert envelope['exit_code'] == 0, envelope
            assert archive.getmember('outputs/result.rmeta').size > 0
        print(
            json.dumps(
                {'case': 'service', 'unprivileged_client': True, 'compiled': True}
            ),
            flush=True,
        )

    def stop(self) -> None:
        """Unregister exactly the owned label before its private files are removed."""
        if self.active:
            result = subprocess.run(
                ['launchctl', 'bootout', f'system/{self.label}'],
                capture_output=True,
                text=True,
                timeout=60,
                check=False,
            )
            if result.returncode != 0:
                state = subprocess.run(
                    ['launchctl', 'print', f'system/{self.label}'],
                    capture_output=True,
                    text=True,
                    timeout=10,
                    check=False,
                )
                if (
                    state.returncode != 113
                    or 'Could not find service' not in state.stderr
                ):
                    raise RuntimeError(
                        f'service removal is unconfirmed, preserving {self.root}: {result.stderr}{state.stderr}'
                    )
            self.active = False


def main() -> None:
    """Create one disposable service and always retire it after qualification."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('worker', type=Path)
    parser.add_argument('fixture', type=Path)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    directory = Path(tempfile.mkdtemp(prefix='bsmr-service-', dir='/private/var/db'))
    service = Service(
        root=directory,
        worker=args.worker.resolve(),
        fixture=args.fixture.resolve(),
        compiler=args.compiler.resolve(),
    )
    try:
        service.start()
        service.qualify()
    finally:
        service.stop()
        shutil.rmtree(directory)


if __name__ == '__main__':
    main()
