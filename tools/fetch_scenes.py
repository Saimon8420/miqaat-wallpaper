"""Re-exports the 20 scene photos at up to 3840 × 2160 from the Wikimedia Commons originals.

    python tools/fetch_scenes.py

The 1920 × 1080 pictures in assets/scenes were hand-cropped from the originals. This finds the
same crop in each original (by matching against the current picture) and saves it again at the
highest 16:9 size the original allows, capped at 4K. Originals are cached in tools/raw/scenes.
"""

import io
import json
import pathlib
import urllib.parse
import urllib.request

from PIL import Image, ImageOps

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCENES = ROOT / "assets" / "scenes"
CACHE = ROOT / "tools" / "raw" / "scenes"
UA = {"User-Agent": "miqaat-wallpaper-builder/1.0 (https://github.com/Saimon8420/miqaat-wallpaper)"}
MAX_W, MAX_H = 3840, 2160
# Originals wider than 16:9 whose old 1080p copy was squeezed to fit: full height, cropped from x.
FULL_HEIGHT_FROM_X = {"hagia": 500, "shatgumbad": 164}


def get(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=180) as r:
        return r.read()


def original(source: str, sid: str) -> Image.Image:
    path = CACHE / f"{sid}.orig"
    if not path.exists():
        title = urllib.parse.unquote(source.split("/wiki/", 1)[1])
        q = urllib.parse.urlencode({"action": "query", "format": "json", "titles": title, "prop": "imageinfo", "iiprop": "url"})
        page = list(json.loads(get(f"https://commons.wikimedia.org/w/api.php?{q}"))["query"]["pages"].values())[0]
        path.write_bytes(get(page["imageinfo"][0]["url"]))
    return ImageOps.exif_transpose(Image.open(path)).convert("RGB")


def signature(im: Image.Image) -> list:
    return list(ImageOps.grayscale(im).resize((48, 27), Image.BILINEAR).getdata())


def best_crop(orig: Image.Image, current: Image.Image) -> tuple:
    """The 16:9 box in `orig` that looks most like `current`: full width, any height offset,
    and a few zoom levels in case the original was trimmed at the sides too."""
    w, h = orig.size
    target = signature(current)
    small = orig.copy()
    small.thumbnail((640, 640))
    k = w / small.width
    best = None
    for zoom in (1.0, 0.95, 0.9, 0.85, 0.8):
        cw = int(small.width * zoom)
        ch = int(cw * 9 / 16)
        if ch > small.height:
            ch = small.height
            cw = int(ch * 16 / 9)
        for x in range(0, small.width - cw + 1, max(1, (small.width - cw) // 12 or 1)):
            for y in range(0, small.height - ch + 1, max(1, (small.height - ch) // 40 or 1)):
                d = sum(abs(a - b) for a, b in zip(target, signature(small.crop((x, y, x + cw, y + ch)))))
                if best is None or d < best[0]:
                    best = (d, (x, y, x + cw, y + ch))
    x0, y0, x1, y1 = best[1]
    return best[0] / len(target), (int(x0 * k), int(y0 * k), int(x1 * k), int(y1 * k))


def main():
    CACHE.mkdir(parents=True, exist_ok=True)
    total = 0
    for sc in json.loads((SCENES / "scenes.json").read_text(encoding="utf-8")):
        sid = sc["id"]
        current = Image.open(SCENES / f"{sid}.jpg").convert("RGB")
        orig = original(sc["source"], sid)
        if sid in FULL_HEIGHT_FROM_X:
            x = FULL_HEIGHT_FROM_X[sid]
            diff, box = 0.0, (x, 0, x + orig.height * 16 // 9, orig.height)
        else:
            diff, box = best_crop(orig, current)
        crop = orig.crop(box)
        scale = min(MAX_W / crop.width, MAX_H / crop.height, 1.0)
        out = crop.resize((round(crop.width * scale) // 2 * 2, round(crop.width * scale * 9 / 16) // 2 * 2), Image.LANCZOS)
        buf = io.BytesIO()
        out.save(buf, "JPEG", quality=84, optimize=True, progressive=True, subsampling=2)
        if diff > 20:
            print(f"!! {sid}: crop match is weak ({diff:.1f}); kept the old picture")
            continue
        (SCENES / f"{sid}.jpg").write_bytes(buf.getvalue())
        total += len(buf.getvalue())
        print(f"{sid:14} original {orig.width}×{orig.height} → {out.width}×{out.height}  {len(buf.getvalue()) // 1024} KB  (match {diff:.1f})")
    print(f"total {total // 1024 // 1024} MB")


if __name__ == "__main__":
    main()
