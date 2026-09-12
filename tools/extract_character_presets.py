from pathlib import Path
import json

root = Path.home() / "Games/ffxiv/game/My Games/FINAL FANTASY XIV - A Realm Reborn"
out = Path("target/character-presets.json")
rows = []
for p in sorted(root.glob("FFXIV_CHARA_*.dat")):
    b = p.read_bytes()
    if b[:4] != bytes.fromhex("14ff1320") or len(b) < 42:
        continue
    rows.append({"file": p.name, "customize": list(b[16:42]), "hex": b[16:42].hex()})
out.write_text(json.dumps(rows, indent=2))
print(out, len(rows))
