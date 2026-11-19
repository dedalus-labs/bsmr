# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies source archives with foreign owners through the native extraction rule.

import argparse
import io
import json
from pathlib import Path
import tarfile
import tempfile

from macros import initialize, run


def main() -> None:
    """Preserve source contents and executable modes without importing archived owners."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-archive-') as temporary:
        project = Path(temporary) / 'project'
        project.mkdir()
        try:
            initialize(project, binary, runtime)
            with tarfile.open(project / 'source.tar.gz', 'w:gz') as archive:
                for uid, gid in [(501, 20), (1000, 1000)]:
                    member = tarfile.TarInfo(f'crate/{uid}')
                    member.uid, member.gid, member.mode = uid, gid, 0o755
                    member.size = 7
                    archive.addfile(member, io.BytesIO(b'source\n'))
            (project / 'BUILD.bsmr').write_text(
                'extract_archive(name="source", src="source.tar.gz", strip_prefix="crate")\n'
            )
            result = run(project, binary, 'build', ':source', '--sandbox', '--show-full-json-output')
            assert result.returncode == 0, result.stderr
            output = Path(next(iter(json.loads(result.stdout).values())))
            for uid in [501, 1000]:
                path = output / str(uid)
                assert path.read_bytes() == b'source\n'
                assert path.stat().st_mode & 0o777 == 0o755
            print('ok  source archives: foreign owners, exact contents, executable modes')
        finally:
            if (project / '.bsmr').exists():
                result = run(project, binary, 'kill')
                assert result.returncode == 0, result.stderr


if __name__ == '__main__':
    main()
