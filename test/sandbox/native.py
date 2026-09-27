# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Exercise native BSMR action execution and cache identity through the installed worker."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def command(
    binary: Path, arguments: list[str], directory: Path
) -> subprocess.CompletedProcess[str]:
    """Run the real engine with a bounded lifetime and retained diagnostics."""
    result = subprocess.run(
        [str(binary), *arguments],
        cwd=directory,
        env={**os.environ, 'BSMR_LOCAL_CACHE_DIR': str(directory.parent / 'cache')},
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f'native engine failed: {result.stdout}{result.stderr}')
    return result


def build(binary: Path, directory: Path, phase: str, value: str) -> None:
    """Check the artifact and actual execution trace, including zero-action DICE reuse."""
    result = command(
        binary=binary,
        arguments=[
            'build',
            '//:copy',
            '--sandbox',
            '--console',
            'simple',
            '--show-full-json-output',
        ],
        directory=directory,
    )
    outputs = json.loads(result.stdout)
    assert len(outputs) == 1
    path = Path(next(iter(outputs.values())))
    assert path.read_text() == value, phase
    trace = re.search(r'Build ID: ([a-f0-9-]+)', result.stderr)
    assert trace is not None, result.stderr
    actions = command(
        binary=binary,
        arguments=[
            'log',
            'what-ran',
            '--trace-id',
            trace[1],
            '--format',
            'json',
            '--filter-category',
            'native_copy',
            '--no-remote',
        ],
        directory=directory,
    )
    executors = [
        json.loads(line)['reproducer']['executor']
        for line in actions.stdout.splitlines()
    ]
    expected = {'cold': ['Local'], 'warm': [], 'edit': ['Local'], 'clone': ['Cache']}
    assert executors == expected[phase], (phase, executors)
    print(
        json.dumps(
            {
                'case': 'native_engine',
                'phase': phase,
                'value': value,
                'executors': executors,
            }
        ),
        flush=True,
    )


def main() -> None:
    """Require correct cold execution, cache reuse and source-edit invalidation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('socket', type=Path)
    parser.add_argument('project', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = args.project.resolve()
    root.mkdir()
    command(binary=binary, arguments=['init'], directory=root)
    (root / '.bsmr.local').write_text(
        f'[sandbox]\nbackend=native\nlauncher_socket={args.socket}\n[bsmr]\ndefault_allow_cache_upload=true\n'
    )
    (root / 'rule.bzl').write_text("""def impl(ctx):
    output = ctx.actions.declare_output("result")
    ctx.actions.run(cmd_args("/probe", "copy", ctx.attrs.source, output.as_output()), category = "native_copy", allow_cache_upload = True)
    return [DefaultInfo(default_output = output)]
copy = rule(impl = impl, attrs = {"source": attrs.source()})
""")
    (root / 'BUILD.bsmr').write_text(
        'load(":rule.bzl", "copy")\ncopy(name="copy", source="input")\n'
    )
    source = root / 'input'
    source.write_text('first')
    clone = root.with_name('clone')
    try:
        for phase, value in [('cold', 'first'), ('warm', 'first'), ('edit', 'second')]:
            source.write_text(value)
            build(binary=binary, directory=root, phase=phase, value=value)
        shutil.copytree(
            root, clone, symlinks=True, ignore=shutil.ignore_patterns('bsmr-out')
        )
        build(binary=binary, directory=clone, phase='clone', value='second')
    finally:
        for directory in [root, clone]:
            if directory.exists():
                command(binary=binary, arguments=['kill'], directory=directory)


if __name__ == '__main__':
    main()
