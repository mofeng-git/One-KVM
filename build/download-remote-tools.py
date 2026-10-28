#!/usr/bin/env python3
"""Fetch the pinned EasyFrp, EasyTier and frp clients for full Docker images."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import struct
import tarfile
import tempfile
import urllib.request
import zipfile


def install(platform, output):
    architecture = {'linux/amd64': 'x86_64', 'linux/arm64': 'aarch64',
                    'linux/arm/v7': 'arm'}[platform]
    lock = json.loads(Path(__file__).with_name('remote-access.lock.json').read_text())
    output.mkdir(parents=True, exist_ok=True)
    expected = {'arm': (1, 40), 'aarch64': (2, 183), 'x86_64': (2, 62)}[architecture]
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({'http': os.environ['CURL_PROXY'], 'https': os.environ['CURL_PROXY']})
    ) if os.environ.get('CURL_PROXY') else urllib.request.build_opener()
    # Validate the entire download before replacing any staged executable.
    with tempfile.TemporaryDirectory(prefix='one-kvm-remote-tools-') as temporary:
        staging = Path(temporary)
        for spec in lock['tools'].values():
            asset = spec['assets'][architecture]
            archive = staging / asset['archive']
            prefix = os.environ.get('GHPROXY_PREFIX', '').rstrip('/')
            url = prefix + '/' + asset['url'] if prefix else asset['url']
            with opener.open(url, timeout=120) as response:
                with archive.open('wb') as stream:
                    shutil.copyfileobj(response, stream)
            if hashlib.sha256(archive.read_bytes()).hexdigest() != asset['sha256']:
                raise ValueError('release checksum differs: ' + archive.name)
            for name in spec['binaries']:
                if zipfile.is_zipfile(archive):
                    with zipfile.ZipFile(archive) as source:
                        matches = [item for item in source.infolist()
                                   if not item.is_dir() and PurePosixPath(item.filename).name == name]
                        if len(matches) != 1:
                            raise ValueError('expected one executable: ' + name)
                        data = source.read(matches[0])
                else:
                    with tarfile.open(archive, 'r:gz') as source:
                        matches = [item for item in source if item.isfile() and PurePosixPath(item.name).name == name]
                        if len(matches) != 1:
                            raise ValueError('expected one executable: ' + name)
                        data = source.extractfile(matches[0]).read()
                if (data[:4] != b'\x7fELF' or data[5] != 1
                        or (data[4], struct.unpack('<H', data[18:20])[0]) != expected):
                    raise ValueError('executable architecture differs: ' + name)
                path = staging / name
                path.write_bytes(data)
                path.chmod(0o755)
        for spec in lock['tools'].values():
            for name in spec['binaries']:
                shutil.copy2(staging / name, output / name)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', required=True, choices=('linux/amd64', 'linux/arm64', 'linux/arm/v7'))
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    install(args.platform, args.output)
