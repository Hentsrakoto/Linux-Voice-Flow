#!/usr/bin/env python3
"""Explicit, pinned model installation. Never imported by application startup."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import time

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {'model.bin', 'config.json', 'tokenizer.json', 'vocabulary.txt'}


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def safe_directory(path):
    for part in (path, *path.parents):
        if part.is_symlink():
            raise ValueError(f'Symlink refused: {part}')
    if path.exists() and not path.is_dir():
        raise ValueError(f'Not a directory: {path}')


def regular(path):
    return not path.is_symlink() and path.exists() and stat.S_ISREG(path.stat().st_mode)


def verify(directory, model):
    safe_directory(directory)
    receipt_path = directory / 'installation.json'
    if not regular(receipt_path):
        raise ValueError('Missing installation receipt; existing model is preserved')
    receipt = json.loads(receipt_path.read_text())
    if receipt.get('repository') != model['repository'] or receipt.get('revision') != model['revision'] or receipt.get('complete') is not True:
        raise ValueError('Unrecognized installation; existing model is preserved')
    files = receipt['files']
    if not REQUIRED <= files.keys():
        raise ValueError('Incomplete manifest')
    for name, expected in files.items():
        if Path(name).name != name:
            raise ValueError('Invalid manifest path')
        path = directory / name
        if not regular(path) or path.stat().st_size != expected['size'] or digest(path) != expected['sha256']:
            raise ValueError(f'Invalid local asset: {name}')
    return receipt


def install(model, root, api=None, download=None):
    root = Path(root).absolute()
    safe_directory(root)
    root.mkdir(parents=True, exist_ok=True)
    if not re.fullmatch(r'[a-z0-9-]+', model['id']) or not re.fullmatch(r'[a-f0-9]{40}', model['revision']):
        raise ValueError('Invalid pinned catalog entry')
    destination = root / model['id']
    if destination.exists() or destination.is_symlink():
        receipt = verify(destination, model)
        print(f"{model['id']}: verified, already installed", flush=True)
        return receipt
    if api is None:
        from huggingface_hub import HfApi, hf_hub_download
        api, download = HfApi(token=False), hf_hub_download
    metadata = api.model_info(model['repository'], revision=model['revision'], files_metadata=True, token=False)
    if metadata.sha != model['revision']:
        raise ValueError('Upstream revision mismatch')
    assets = [s for s in metadata.siblings if s.rfilename in REQUIRED or s.rfilename in {'README.md', 'LICENSE', 'LICENSE.txt', 'preprocessor_config.json'}]
    if not REQUIRED <= {s.rfilename for s in assets}:
        raise ValueError('Upstream model incomplete')
    staging = root / ('.' + model['id'] + '.staging')
    safe_directory(staging)
    staging.mkdir(exist_ok=True)
    # Refuse symlinks, including any leftover downloader metadata.
    for path in staging.rglob('*'):
        if path.is_symlink():
            raise ValueError(f'Symlink refused: {path}')
    manifest = {}
    started = time.monotonic()
    for asset in assets:
        name = asset.rfilename
        print(f"{model['id']}: downloading/verifying {name} ({asset.size} bytes)", flush=True)
        download(repo_id=model['repository'], filename=name, revision=model['revision'], local_dir=str(staging), token=False)
        path = staging / name
        if not regular(path) or path.stat().st_size != asset.size:
            raise ValueError(f'Invalid asset: {name}')
        sha = digest(path)
        lfs = asset.lfs
        upstream_sha = (lfs.get('sha256') if isinstance(lfs, dict) else getattr(lfs, 'sha256', None)) if lfs else None
        if name == 'model.bin' and not upstream_sha:
            raise ValueError('Missing official weights checksum')
        if upstream_sha and sha != upstream_sha:
            raise ValueError(f'Checksum mismatch: {name}')
        manifest[name] = {'size': asset.size, 'sha256': sha, 'upstream_sha256': upstream_sha}
    receipt = {'complete': True, 'repository': model['repository'], 'revision': model['revision'], 'license': 'MIT', 'files': manifest, 'elapsed_seconds': round(time.monotonic() - started, 3)}
    temporary = staging / 'installation.json.tmp'
    temporary.write_text(json.dumps(receipt, indent=2) + '\n')
    os.replace(temporary, staging / 'installation.json')
    verify(staging, model)
    # Never replace an existing destination, even one created during download.
    if destination.exists() or destination.is_symlink():
        raise ValueError('Destination appeared during installation; preserved')
    staging.rename(destination)
    print(f"{model['id']}: installed {sum(f['size'] for f in manifest.values())} bytes in {receipt['elapsed_seconds']}s", flush=True)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('models', nargs='*', help='Catalog IDs; defaults to all')
    parser.add_argument('--verify-only', action='store_true', help='Check receipts offline without downloading')
    args = parser.parse_args()
    catalog = json.loads((ROOT / 'config/models.json').read_text())['models']
    selected = args.models or [m['id'] for m in catalog]
    if set(selected) - {m['id'] for m in catalog}:
        parser.error('Unknown model ID')
    for model in catalog:
        if model['id'] in selected:
            if args.verify_only:
                verify(ROOT / 'models' / model['id'], model)
                print(f"{model['id']}: verified offline")
            else:
                install(model, ROOT / 'models')


if __name__ == '__main__':
    main()
