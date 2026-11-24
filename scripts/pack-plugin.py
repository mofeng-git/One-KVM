#!/usr/bin/env python3
"""Wrap one executable and its versioned JSON manifest in a .onepkg file."""

import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile

MAGIC = b"ONEPKG1\n"
MAX_MANIFEST = 128 * 1024
MAX_PACKAGE = 192 * 1024 * 1024

ARCHES = {"x86_64": "x86_64", "aarch64": "aarch64", "armv7": "arm", "i686": "x86"}


def package(manifest_path, executable, target, output):
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    arch = ARCHES.get(target.split("-", 1)[0])
    os_name = next((name for name in ("linux", "windows") if name in target.split("-")), None)
    if not arch or not os_name:
        raise ValueError(f"unsupported plugin target: {target}")
    manifest.update(arch=arch, os=os_name)
    metadata = json.dumps(manifest, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    size = executable.stat().st_size
    if not 0 < size or len(metadata) > MAX_MANIFEST or len(MAGIC) + 4 + len(metadata) + size > MAX_PACKAGE:
        raise ValueError("invalid manifest or executable length")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as file:
        temporary = Path(file.name)
        try:
            file.write(MAGIC + struct.pack(">I", len(metadata)) + metadata)
            with executable.open("rb") as source:
                shutil.copyfileobj(source, file)
            file.close()
            temporary.chmod(0o644)
            os.replace(temporary, output)
        finally:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    package(args.manifest, args.binary, args.target, args.output)
