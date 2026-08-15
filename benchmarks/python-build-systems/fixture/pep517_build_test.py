# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies the integrity gate inside the Bazel PEP 517 benchmark control.

"""Verify the Bazel control measures the same wheel-integrity work as BSMR."""

from __future__ import annotations

import base64
import csv
import hashlib
import io
import tempfile
import unittest
import zipfile
from pathlib import Path

from pep517_build import _validate_wheel


def _wheel(path: Path) -> Path:
    """Write one minimal wheel with a complete strong RECORD."""
    files = {
        "demo.py": b"VALUE = 1\n",
        "demo-1.dist-info/METADATA": (
            b"Metadata-Version: 2.5\nName: demo\nVersion: 1\n"
        ),
        "demo-1.dist-info/WHEEL": (
            b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n"
        ),
    }
    rows = [
        [
            name,
            "sha256="
            + base64.urlsafe_b64encode(hashlib.sha256(data).digest())
            .rstrip(b"=")
            .decode(),
            str(len(data)),
        ]
        for name, data in files.items()
    ]
    record = "demo-1.dist-info/RECORD"
    rows.append([record, "", ""])
    output = io.StringIO()
    csv.writer(output, lineterminator="\n").writerows(rows)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in {**files, record: output.getvalue().encode()}.items():
            archive.writestr(name, data)
    return path


class WheelValidationTest(unittest.TestCase):
    """Exercise the control's measured artifact verification."""

    def test_valid_wheel_passes(self) -> None:
        """A complete wheel with strong RECORD identities is accepted."""
        with tempfile.TemporaryDirectory() as temporary:
            _validate_wheel(_wheel(Path(temporary) / "demo-1-py3-none-any.whl"))

    def test_payload_mutation_fails(self) -> None:
        """The control must reject bytes that no longer match RECORD."""
        with tempfile.TemporaryDirectory() as temporary:
            wheel = _wheel(Path(temporary) / "demo-1-py3-none-any.whl")
            with zipfile.ZipFile(wheel) as archive:
                files = {name: archive.read(name) for name in archive.namelist()}
            files["demo.py"] = b"VALUE = 2\n"
            with zipfile.ZipFile(wheel, "w", zipfile.ZIP_DEFLATED) as archive:
                for name, data in files.items():
                    archive.writestr(name, data)

            with self.assertRaisesRegex(RuntimeError, "hash mismatch"):
                _validate_wheel(wheel)


if __name__ == "__main__":
    unittest.main()
