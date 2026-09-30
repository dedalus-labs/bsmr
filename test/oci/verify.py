# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Verify exported OCI bytes and report their effective filesystem for differential tests."""

import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import sys
import tarfile


def blob(layout: Path, descriptor: dict) -> bytes:
    """Read a descriptor's exact payload after verifying its size and SHA-256."""
    algorithm, digest = descriptor["digest"].split(":", 1)
    assert algorithm == "sha256", algorithm
    assert len(digest) == 64 and all(char in "0123456789abcdef" for char in digest)
    data = (layout / "blobs" / algorithm / digest).read_bytes()
    assert len(data) == descriptor["size"], "descriptor size mismatch"
    assert hashlib.sha256(data).hexdigest() == digest, "descriptor digest mismatch"
    return data


def safe_path(name: str) -> str:
    """Return an image-relative path, rejecting archive paths that escape its root."""
    path = PurePosixPath(name)
    assert not path.is_absolute() and ".." not in path.parts, name
    return str(path)


def verify(layout: Path) -> dict:
    """Check one platform image and independently apply its ordered tar changesets."""
    assert json.loads((layout / "oci-layout").read_text())["imageLayoutVersion"] == "1.0.0"
    index = json.loads((layout / "index.json").read_text())
    assert index["schemaVersion"] == 2 and len(index["manifests"]) == 1
    descriptor = index["manifests"][0]
    manifest = json.loads(blob(layout, descriptor))
    if manifest["mediaType"] == "application/vnd.oci.image.index.v1+json":
        assert len(manifest["manifests"]) == 1
        descriptor = manifest["manifests"][0]
        manifest = json.loads(blob(layout, descriptor))
    assert manifest["schemaVersion"] == 2
    config = json.loads(blob(layout, manifest["config"]))
    assert config["rootfs"]["type"] == "layers"
    assert len(manifest["layers"]) == len(config["rootfs"]["diff_ids"])
    files = {}
    for layer, expected in zip(manifest["layers"], config["rootfs"]["diff_ids"]):
        data = blob(layout, layer)
        if layer["mediaType"].endswith("+gzip"):
            data = gzip.decompress(data)
        else:
            assert layer["mediaType"] == "application/vnd.oci.image.layer.v1.tar"
        assert "sha256:" + hashlib.sha256(data).hexdigest() == expected, "layer diffID mismatch"
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
            for entry in archive:
                name = safe_path(entry.name)
                if name == ".":
                    continue
                assert not PurePosixPath(name).name.startswith(".wh."), "whiteouts require a separate fixture"
                metadata = {"mode": entry.mode, "uid": entry.uid, "gid": entry.gid}
                if entry.isfile():
                    stream = archive.extractfile(entry)
                    assert stream is not None
                    content = stream.read()
                    files[name] = {"type": "file", "sha256": hashlib.sha256(content).hexdigest(), **metadata}
                elif entry.isdir():
                    files[name] = {"type": "directory", **metadata}
                elif entry.issym():
                    files[name] = {"type": "symlink", "target": entry.linkname, **metadata}
                elif entry.islnk():
                    files[name] = dict(files[safe_path(entry.linkname)])
                else:
                    raise AssertionError(f"unsupported fixture entry: {name}")
    return {"digest": descriptor["digest"], "config": config, "layers": manifest["layers"], "files": files}


if __name__ == "__main__":
    print(json.dumps(verify(Path(sys.argv[1])), sort_keys=True))
