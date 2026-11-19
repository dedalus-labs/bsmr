# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Checks that ambient Git settings cannot rewrite a pinned source artifact.

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class GitSourceTest(unittest.TestCase):
    def test_invariant_pinned_source_ignores_ambient_filters_and_hooks(self) -> None:
        """The same commit produces the same source under hostile caller configuration."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            origin = root / "origin"
            origin.mkdir()
            env = os.environ | {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
            for args in [
                ["init", "--quiet", "--template="],
                ["config", "user.name", "Fixture"],
                ["config", "user.email", "fixture@example.invalid"],
            ]:
                subprocess.run(["git", *args], cwd=origin, env=env, check=True, capture_output=True)
            source = b"pub fn value() -> u32 { 42 }\n"
            (origin / "source.rs").write_bytes(source)
            (origin / ".gitattributes").write_text("*.rs filter=ambient\n")
            subprocess.run(["git", "add", "."], cwd=origin, env=env, check=True)
            subprocess.run(["git", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture"], cwd=origin, env=env, check=True)
            revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=origin, env=env, text=True).strip()
            hook = root / "template/hooks/post-checkout"
            hook.parent.mkdir(parents=True)
            hook.write_text(f'#!/bin/sh\ntouch "{root / "executed"}"\n')
            hook.chmod(0o700)
            config = root / "config"
            config.write_text('[filter "ambient"]\nsmudge = sed s/42/13/g\n')
            env.update({"GIT_CONFIG_GLOBAL": str(config), "GIT_TEMPLATE_DIR": str(hook.parent.parent)})
            output = root / "output"
            tool = Path(__file__).resolve().parents[1] / "git_fetch.py"
            result = subprocess.run([
                sys.executable, tool, "--git-dir", root / "git", "--work-tree", output,
                "--repo", str(origin), "--rev", revision,
            ], env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((output / "source.rs").read_bytes(), source)
            self.assertFalse((root / "executed").exists())


if __name__ == "__main__":
    unittest.main()
