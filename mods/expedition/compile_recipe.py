# SPDX-License-Identifier: MPL-2.0
"""Compile structural edits from a player's original and locally authored expansion.

Only differences are distributed. Unchanged dictionaries, original strings and
map cells are read from the player's copy by the emulator.
"""
import argparse
import base64
import gzip
import hashlib
import json
import plistlib
import re
import struct
import zlib
from pathlib import Path
from zipfile import ZipFile
from resource_io import resource_key, crypt_resources


def identity(value):
    if isinstance(value, dict):
        return tuple(sorted((key, identity(v)) for key, v in value.items())), 'dictionary'
    if isinstance(value, list):
        return tuple(map(identity, value)), 'array'
    return value, type(value).__name__


def register(index, value, path):
    if isinstance(value, (dict, list)) or isinstance(value, str) and len(value) >= 24:
        index.setdefault(identity(value), path)
    for k, v in (value.items() if isinstance(value, dict) else enumerate(value) if isinstance(value, list) else ()):
        register(index, v, [*path, k])


def expression(new, old, refs):
    match = refs.get(identity(new))
    if match is not None and (old is None or not isinstance(new, type(old)) or isinstance(new, str)):
        return {'ref': match}
    if isinstance(new, dict):
        original = old if isinstance(old, dict) else {}
        changes = {k: expression(v, original.get(k), refs) for k, v in new.items()
                   if k not in original or identity(v) != identity(original[k])}
        return {'map': [changes, sorted(original.keys() - new.keys())]}
    if isinstance(new, list):
        original = old if isinstance(old, list) else []
        lo = hi = 0
        common = min(len(original), len(new))
        while lo < common and identity(original[lo]) == identity(new[lo]):
            lo += 1
        while hi < common - lo and identity(original[-1-hi]) == identity(new[-1-hi]):
            hi += 1
        end = len(original) - hi
        edits = []
        for i in range(lo, len(new) - hi):
            ref = refs.get(identity(new[i]))
            edits.append({'ref': ref} if ref is not None else expression(new[i], original[i] if i < end else None, refs))
        return {'array': [lo, end, edits]}
    if isinstance(new, bytes):
        raise ValueError('Unexpected new binary game data')
    return {'value': new}


def decode(files, key):
    result = {}; encrypted = []
    for n, raw in files.items():
        if raw.startswith((b'bplist', b'<?xml')):
            result[n] = plistlib.loads(raw)
        else:
            encrypted.append((n, raw))
    for i in range(0, len(encrypted), 16):
        group = encrypted[i:i+16]
        for (n, _), plain in zip(group, crypt_resources(key, [raw for _, raw in group])):
            result[n] = plistlib.loads(gzip.decompress(plain))
    return result


LAYERS = re.compile(rb'<layer name="([^"]+)"[^>]*>\s*<data encoding="base64" compression="zlib">\s*([\w+/=\s]+?)\s*</data>')


def layers(raw):
    return {name.decode(): list(struct.iter_unpack('<I', zlib.decompress(base64.b64decode(data))))
            for name, data in LAYERS.findall(raw)}


def compile_recipe(original, expanded, output):
    with ZipFile(original) as stock, ZipFile(expanded) as mod:
        prefix, = [n[:-len('overworld.tmx')] for n in stock.namelist() if n.endswith('.app/overworld.tmx')]
        key = resource_key(stock, prefix[:-1])
        names = set(stock.namelist())
        changed = sorted(n[len(prefix):] for n in mod.namelist() if n.startswith(prefix)
                         and (n not in names or stock.getinfo(n).CRC != mod.getinfo(n).CRC))
        changed.remove('overworld.tmx')
        assert all(n.endswith(('.plist', '.strings')) for n in changed)
        sources = sorted({n for n in changed if prefix+n in names} | {'GrayrockCave.plist','over-38x97.plist'})
        raw = {n: stock.read(prefix+n) for n in sources}
        old = decode(raw, key)
        after = decode({n: mod.read(prefix+n) for n in changed}, key)
        refs = {}
        for n in sources:
            register(refs, old[n], [n])
        resources = {n: expression(after[n], old.get(n), refs) for n in changed}
        terrain = stock.read(prefix+'overworld.tmx')
        original_layers, mod_layers = layers(terrain), layers(mod.read(prefix+'overworld.tmx'))
        cells = {n: [[i, value[0]] for i, value in enumerate(layer) if value != original_layers[n][i]]
                 for n, layer in mod_layers.items() if layer != original_layers[n]}
        hashes = {n: hashlib.sha256(v).hexdigest() for n, v in raw.items()}
        hashes['overworld.tmx'] = hashlib.sha256(terrain).hexdigest()
        # A flat instruction table avoids JSON recursion limits on long quest conditions.
        edits, ids = [], {}
        def intern(expr):
            if 'map' in expr:
                changes, removed = expr['map']
                expr = {'map': [{k: intern(v) for k, v in changes.items()}, removed]}
            elif 'array' in expr:
                start, end, items = expr['array']
                expr = {'array': [start, end, [intern(v) for v in items]]}
            key = json.dumps(expr, ensure_ascii=False, separators=(',', ':'))
            if key not in ids:
                ids[key] = len(edits)
                edits.append(expr)
            return ids[key]
        roots = {k: intern(v) for k, v in resources.items()}
        recipe = dict(sources=hashes, resources=roots, edits=edits, cells=cells)
        output.write_bytes(gzip.compress(json.dumps(recipe, ensure_ascii=False, separators=(',', ':')).encode(), mtime=0))
        print(f'{len(resources)} resources, {sum(map(len,cells.values()))} edited cells, {output.stat().st_size} compressed bytes')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['original', 'expanded', 'output']:
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    compile_recipe(args.original, args.expanded, args.output)
