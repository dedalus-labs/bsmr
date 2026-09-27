# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Exercise native BSMR action execution and cache identity through the installed worker."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess


def command(
    binary: Path, arguments: list[str], directory: Path
) -> subprocess.CompletedProcess[str]:
    """Run the real engine with a bounded lifetime and retained diagnostics."""
    result = subprocess.run(
        [str(binary), *arguments],
        cwd=directory,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f'native engine failed: {result.stdout}{result.stderr}')
    return result


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
    try:
        for phase, value in [('cold', 'first'), ('warm', 'first'), ('edit', 'second')]:
            source.write_text(value)
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
                directory=root,
            )
            outputs = json.loads(result.stdout)
            assert len(outputs) == 1
            path = Path(next(iter(outputs.values())))
            assert path.read_text() == value, phase
            if phase == 'warm':
                assert 'cached: 1' in result.stderr, result.stderr
            print(
                json.dumps({'case': 'native_engine', 'phase': phase, 'value': value}),
                flush=True,
            )
    finally:
        command(binary=binary, arguments=['kill'], directory=root)


if __name__ == '__main__':
    main()
