"""ソースコード中で使っている文字だけを含む日本語フォントを作る。

使い方: PYTHONPATH=<fonttools> python3 tools/subset_font.py
元フォント: Noto Sans CJK JP Bold (SIL Open Font License 1.1)
"""
import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTCollection

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc"

chars = set(chr(c) for c in range(0x20, 0x7F))
chars |= set("、。「」『』（）！？・…〜ー―◆▼▶△○×：；％＋－ 　")
for p in (ROOT / "src").rglob("*.rs"):
    chars |= set(p.read_text(encoding="utf-8"))
chars = {c for c in chars if c.isprintable() or c == " "}

coll = TTCollection(SRC)
font = None
for f in coll.fonts:
    names = [n.toUnicode() for n in f["name"].names if n.nameID == 1]
    if any("CJK JP" in n for n in names):
        font = f
        break
if font is None:
    sys.exit("JP font not found in collection")

opts = subset.Options()
opts.layout_features = ["*"]
opts.name_IDs = ["*"]
opts.notdef_outline = True
sub = subset.Subsetter(opts)
sub.populate(text="".join(sorted(chars)))
sub.subset(font)
out = ROOT / "assets" / "fonts" / "game.otf"
font.save(out)
print(f"{len(chars)} chars -> {out} ({out.stat().st_size // 1024} KiB)")
