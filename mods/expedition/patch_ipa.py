# SPDX-License-Identifier: MPL-2.0
"""Build a separate local Hunter Island 2.02 archive from an owned retail IPA."""
import argparse
from base64 import b64decode, b64encode
from copy import copy
import gzip
import hashlib
from quest_language import expedition_tables
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import tempfile
from zipfile import ZipFile, ZIP_DEFLATED
import zlib
from resource_io import resource_key, crypt_resources
from native_campaign import Campaign

CATALOGS = ['A1toA18','A19toA29','A30toA47','A48toA67','A68toA88','A89toA97','DungeonAndCity']
SOURCE = Path(__file__).with_name('campaign.json')
def tile(p): return 'over-%dx%d' % tuple(p)

def terrain(raw, branches):
    if hashlib.sha256(raw).hexdigest() != '263b05c2e50d052ffc8fa6e36891f2d25f13521edae13c208b57fbee58537f72':
        raise ValueError('Expected the unmodified retail overworld.')
    # These entrances and the meadow use regional scenery from the retail atlas.
    # Arborun's original anchor is water until its story overlay appears.
    end_scenery = {(125,108):(123,109), (126,73):(38,97), (24,16):(34,11), (16,43):(128,79)}
    for layer in ['Ground','Terrain','Top','Buildings','connections','pois']:
        pattern = rb'<layer name="' + layer.encode() + rb'"[^>]*>\s*<data encoding="base64" compression="zlib">\s*([\w+/=\s]+?)\s*</data>'
        match = re.search(pattern,raw)
        if not match: raise ValueError(layer)
        data = bytearray(zlib.decompress(b64decode(match[1])))
        if len(data)!=300*280*4: raise ValueError('Tile dimensions')
        def offsets(point):
            x,y = point
            return [4*((2*y+j)*300+2*x+i) for j in range(2) for i in range(2)]
        for branch in branches:
            anchor = [struct.unpack_from('<I',data,i)[0] for i in offsets(branch['anchor'])]
            for key in ['via','end']:
                dest = offsets(branch[key])
                if layer in ('pois','connections') and any(struct.unpack_from('<I',data,i)[0] for i in dest):
                    raise ValueError('Occupied '+layer+' at '+str(branch[key]))
                if layer=='pois' and key=='via': continue
                cells = anchor if layer in ('Ground','connections','pois') else (0,0,0,0)
                end = tuple(branch['end'])
                if layer in ('Ground','Terrain','Top') and end in end_scenery:
                    if key=='end' or (end==(125,108) and layer=='Ground'):
                        cells=[struct.unpack_from('<I',data,i)[0] for i in offsets(end_scenery[end])]
                for offset,gid in zip(dest,cells): struct.pack_into('<I',data,offset,gid)
        raw=raw[:match.start(1)]+b64encode(zlib.compress(data))+raw[match.end(1):]
    return raw

def greenpath_crossing(raw):
    # The story overlay paints a river over the new eastbound path. At offset
    # {244,219}, the river crossing at world (124,108) starts at local (4,20).
    # Put the original east-west timber bridge on top of the river and banks.
    pattern = rb'<layer name="Top"[^>]*>\s*<data encoding="base64" compression="zlib">\s*([\w+/=\s]+?)\s*</data>'
    match = re.search(pattern, raw)
    data = bytearray(zlib.decompress(b64decode(match[1])))
    if len(data) != 8 * 24 * 4:
        raise ValueError('Unsupported Greenpath overlay')
    for y, row in enumerate(((291, 292), (314, 315)), 20):
        for x, gid in enumerate(row, 4):
            offset = 4 * (y * 8 + x)
            if struct.unpack_from('<I', data, offset)[0]:
                raise ValueError('Occupied Greenpath crossing')
            struct.pack_into('<I', data, offset, gid)
    return raw[:match.start(1)] + b64encode(zlib.compress(data)) + raw[match.end(1):]

def build(source,destination):
    source,destination=Path(source).resolve(),Path(destination).resolve()
    if source==destination or destination.exists(): raise ValueError('Choose a new output path. The original and existing builds are protected.')
    config=json.loads(SOURCE.read_text('utf-8'))
    with ZipFile(source) as original:
        names=original.namelist()
        bundle,= [n.rsplit('/',1)[0]+'/' for n in names if re.fullmatch(r'Payload/[^/]+\.app/overworld\.tmx',n)]
        secret=resource_key(original,bundle[:-1])
        members=set(names)
        cached={}
        def load(keys):
            missing=sorted({k for k in keys if k not in cached and bundle+k+'.plist' in members})
            encrypted=[]
            for key in missing:
                raw=original.read(bundle+key+'.plist')
                if raw.startswith((b'bplist',b'<?xml')):cached[key]=plistlib.loads(raw)
                else:encrypted.append((key,raw))
            for i in range(0,len(encrypted),32):
                chunk=encrypted[i:i+32]; plain=crypt_resources(secret,[raw for _,raw in chunk])
                for (key,_),raw in zip(chunk,plain):cached[key]=plistlib.loads(gzip.decompress(raw))
        base={'Towns','Quests','Chats','Overworld','VsRewards','GrayrockCave','over-38x97','Monsters','Combines',*CATALOGS}
        points={s['place'] for q in config['quests'] for s in q['steps']} | {q['offer'] for q in config['quests']} | {h['place'] for h in config['habitats']}
        points.update(tile(b['anchor']) for b in config['branches'])
        points.update(['over-26x111','over-25x20','over-42x47','over-59x17'])
        load(base|points)
        mod=Campaign(config,cached,CATALOGS,load)
        mod.install()
        replacements={bundle+'overworld.tmx':terrain(original.read(bundle+'overworld.tmx'),config['branches']),
                      bundle+'GreenpathOverlay.tmx':greenpath_crossing(original.read(bundle+'GreenpathOverlay.tmx'))}
        resources=sorted(mod.modified)
        for i in range(0,len(resources),32):
            chunk=resources[i:i+32]; encrypted=[]
            for k in chunk:
                raw=plistlib.dumps(cached[k],fmt=plistlib.FMT_BINARY,sort_keys=False)
                if k=='Chats':replacements[bundle+k+'.plist']=raw
                else:encrypted.append((k,gzip.compress(raw,mtime=0)))
            if encrypted:
                for (k,_),raw in zip(encrypted,crypt_resources(secret,[v for _,v in encrypted],encrypt=True)):replacements[bundle+k+'.plist']=raw
        for language, additions in expedition_tables(mod.strings).items():
            n = bundle + language + '.lproj/Localizable.strings'
            table = plistlib.loads(original.read(n))
            if table.keys() & additions.keys():
                raise ValueError('String collision in ' + language)
            table.update(additions)
            replacements[n] = plistlib.dumps(table, fmt=plistlib.FMT_BINARY, sort_keys=False)
        destination.parent.mkdir(parents=True,exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=destination.parent,delete=False,suffix='.importing') as tmp:stage=Path(tmp.name)
        try:
            with ZipFile(stage,'w',ZIP_DEFLATED,compresslevel=1) as out:
                for info in original.infolist():
                    if info.filename in replacements:out.writestr(copy(info),replacements.pop(info.filename))
                    else:
                        with original.open(info) as src,out.open(copy(info),'w') as dst:shutil.copyfileobj(src,dst,1024*1024)
                for n,raw in replacements.items():out.writestr(n,raw)
            with ZipFile(stage) as result:
                if result.testzip():raise ValueError('Archive CRC')
            stage.rename(destination)
        finally:stage.unlink(missing_ok=True)
        print(f'{len(config["quests"])} quest chains, {len(mod.modified)} native resources, {len(mod.strings)} strings -> {destination}')
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('original');parser.add_argument('destination')
    args=parser.parse_args();build(args.original,args.destination)
