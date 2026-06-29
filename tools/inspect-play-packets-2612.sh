#!/usr/bin/env bash
set -euo pipefail

mkdir -p inspect
python3 - <<'PY'
import json, urllib.request
manifest=json.load(urllib.request.urlopen('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
entry=next(v for v in manifest['versions'] if v['id']=='26.1.2')
version=json.load(urllib.request.urlopen(entry['url']))
for key in ('server','server_mappings'):
    item=version['downloads'][key]
    urllib.request.urlretrieve(item['url'], f'inspect/{key}')
    print(key, item['sha1'], item['url'])
PY

mkdir -p inspect/server-unpacked
cd inspect/server-unpacked
jar xf ../server
INNER=$(find META-INF/versions -type f -name '*.jar' | head -n1)
if [[ -z "${INNER}" ]]; then
  echo 'inner server jar not found' >&2
  exit 1
fi
cp "$INNER" ../server-inner.jar
cd ../..

for NAME in \
  ClientboundLoginPacket \
  ClientboundSetDefaultSpawnPositionPacket \
  ClientboundPlayerPositionPacket \
  ServerboundAcceptTeleportationPacket \
  ClientboundKeepAlivePacket \
  ServerboundKeepAlivePacket \
  CommonPlayerSpawnInfo \
  PositionMoveRotation; do
  grep -F "$NAME" inspect/server_mappings || true
done > inspect/mapping-hits.txt

python3 - <<'PY'
from pathlib import Path
hits=Path('inspect/mapping-hits.txt').read_text().splitlines()
classes=[]
for line in hits:
    if ' -> ' in line and line.endswith(':') and not line.startswith(' '):
        left,right=line[:-1].split(' -> ',1)
        classes.append((left,right))
Path('inspect/classes.tsv').write_text(''.join(f'{a}\t{b}\n' for a,b in classes))
PY

: > inspect/javap.txt
while IFS=$'\t' read -r NAMED OBF; do
  [[ -z "$OBF" ]] && continue
  echo "===== $NAMED -> $OBF =====" >> inspect/javap.txt
  javap -classpath inspect/server-inner.jar -p -c -s "$OBF" >> inspect/javap.txt 2>&1 || true
done < inspect/classes.tsv

java -DbundlerMainClass=net.minecraft.data.Main -jar inspect/server --reports
python3 - <<'PY'
import json
p=json.load(open('generated/reports/packets.json'))['play']
wanted={
'minecraft:login','minecraft:set_default_spawn_position','minecraft:player_position',
'minecraft:keep_alive','minecraft:disconnect','minecraft:system_chat'
}
with open('inspect/packet-ids.txt','w') as f:
    for side in ('clientbound','serverbound'):
        f.write(f'[{side}]\n')
        for k,v in p[side].items():
            if k in wanted:
                f.write(f'{k}={v["protocol_id"]}\n')
PY
