"""Downloads the wallpaper's font choices into assets/fonts (all SIL Open Font Licence).

    python fetch_fonts.py

Uses Fontsource's per-script subsets so each file only carries the letters it draws:
Arabic fonts get the Arabic subset, Bangla fonts the Bengali one, English fonts Latin.
"""

import hashlib
import json
import pathlib
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "fonts"
UA = {"User-Agent": "miqaat-wallpaper-builder/1.0"}

# (fontsource id, subset, weights wanted). Missing weights are skipped; DirectWrite picks the nearest.
FONTS = [
    *[(f, "arabic", [400]) for f in ["noto-naskh-arabic", "scheherazade-new", "lateef", "amiri", "ibm-plex-sans-arabic",
                                     "tajawal", "almarai", "readex-pro", "cairo", "noto-kufi-arabic", "reem-kufi"]],
    *[(f, "bengali", [300, 400]) for f in ["noto-sans-bengali", "hind-siliguri", "tiro-bangla", "baloo-da-2"]],
    *[(f, "latin", [200, 300, 400, 600]) for f in ["geist", "manrope", "plus-jakarta-sans", "inter", "outfit", "sora",
                                                   "dm-sans", "urbanist", "fraunces", "playfair-display", "lora", "jetbrains-mono"]],
]


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
        return r.read()


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    total = 0
    for fid, subset, weights in FONTS:
        meta = json.loads(fetch(f"https://api.fontsource.org/v1/fonts/{fid}"))
        assert meta["license"].startswith("OFL"), fid
        seen = set()
        for wt in weights:
            v = meta["variants"].get(str(wt), {}).get("normal", {}).get(subset)
            if not v:
                continue
            out = ROOT / f"{fid}-{wt}.ttf"
            data = out.read_bytes() if out.exists() else fetch(v["url"]["ttf"])
            digest = hashlib.sha1(data).hexdigest()
            if digest in seen:  # variable fonts come back as the same file for every weight
                out.unlink(missing_ok=True)
                continue
            seen.add(digest)
            out.write_bytes(data)
            total += out.stat().st_size
            print(f"{out.name:40} {out.stat().st_size // 1024:5} KB")
    print(f"total {total // 1024} KB")


if __name__ == "__main__":
    main()
