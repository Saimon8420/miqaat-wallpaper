"""Builds assets/content.json, the offline library the wallpaper reads.

    cd tools && npm install && python build_content.py

Downloads (cached in tools/raw): hadith-api editions and Hisn al-Muslim chapters. Prints every
item so a person can check it before release. Nothing here is typed by hand except references.
"""

import json
import pathlib
import re
import subprocess
import sys
import urllib.request

from refs import ADHKAR_CHAPTERS, HADITH, QURAN

ROOT = pathlib.Path(__file__).resolve().parent
RAW = ROOT / "raw"
OUT = ROOT.parent / "assets" / "content.json"
UA = {"User-Agent": "miqaat-wallpaper-builder/1.0"}
sys.stdout.reconfigure(encoding="utf-8")

NAMES = {"bukhari": "Sahih al-Bukhari", "muslim": "Sahih Muslim", "tirmidhi": "Jami' at-Tirmidhi", "nawawi": "An-Nawawi's Forty Hadith",
         "abudawud": "Sunan Abi Dawud", "nasai": "Sunan an-Nasa'i", "ibnmajah": "Sunan Ibn Majah"}


def get(url: str, cache: pathlib.Path) -> str:
    if not cache.exists():
        with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120) as r:
            cache.write_bytes(r.read())
    return cache.read_text(encoding="utf-8-sig")


def clean(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


def arabic_digits(n: int) -> str:
    return "".join(chr(0x0660 + int(c)) for c in str(n))


# ---------------- Quran ----------------
def quran():
    refs = ",".join(f"{s}:{a}-{b}" for s, a, b, _ in QURAN)
    data = json.loads(subprocess.check_output(["node", "quran.mjs", refs], cwd=ROOT))
    items = []
    for s, a, b, tags in QURAN:
        d = data[f"{s}:{a}-{b}"]
        ar = " ".join(f"{clean(v['ar'])} ﴿{arabic_digits(n)}﴾" if b > a else clean(v["ar"]) for n, v in zip(range(a, b + 1), d["verses"]))
        en = " ".join(clean(v["en"]) for v in d["verses"])
        bn = " ".join(clean(v["bn"]) for v in d["verses"])
        ref = f"{d['surahEn']} {s}:{a}" + (f"–{b}" if b > a else "")
        items.append({"kind": "quran", "tags": tags, "ar": ar, "en": en, "bn": bn, "ref": ref})
    return items


# ---------------- Hadith ----------------
_editions = {}


def edition(lang: str, coll: str):
    key = f"{lang}-{coll}"
    if key not in _editions:
        text = get(f"https://cdn.jsdelivr.net/gh/fawazahmed0/hadith-api@1/editions/{key}.min.json", RAW / f"{key}.min.json")
        _editions[key] = json.loads(text)["hadiths"]
    return _editions[key]


def find(lang: str, coll: str, num: int):
    rows = edition(lang, coll)
    if coll == "muslim":
        # standard numbering is the integer part of `arabicnumber` (2691.01 -> 2691); take the first part
        hits = [h for h in rows if "arabicnumber" in h and int(float(h["arabicnumber"])) == num and h["text"].strip()]
        return hits[0] if hits else None
    hits = [h for h in rows if h["hadithnumber"] == num and h["text"].strip()]
    return hits[0] if hits else None


def hadith():
    items = []
    for coll, num, tags in HADITH:
        en, ar, bn = find("eng", coll, num), find("ara", coll, num), find("ben", coll, num)
        if not en or not ar:
            print(f"!! missing {coll} {num}")
            continue
        grades = [f"{g['grade']} ({g['name']})" for g in en.get("grades", [])]
        # Bukhari and Muslim are sahih by consensus; for the others prefer al-Albani's grading.
        albani = [g for g in en.get("grades", []) if "Albani" in g["name"]]
        grade = "Sahih" if coll in ("bukhari", "muslim") else (f"{albani[0]['grade']} (al-Albani)" if albani else (grades[0] if grades else ""))
        items.append({"kind": "hadith", "tags": tags, "long": len(clean(en["text"])) > 380, "ar": clean(ar["text"]), "en": clean(en["text"]), "bn": clean(bn["text"]) if bn else "",
                      "ref": f"{NAMES[coll]} {num}", "grade": grade})
    return items


# ---------------- Hisn al-Muslim ----------------
def adhkar():
    items = []
    for cid, slot in ADHKAR_CHAPTERS.items():
        ar = json.loads(get(f"https://www.hisnmuslim.com/api/ar/{cid}.json", RAW / f"hisn-ar-{cid}.json"))
        en = json.loads(get(f"https://www.hisnmuslim.com/api/en/{cid}.json", RAW / f"hisn-en-{cid}.json"))
        ar_rows, en_rows = list(ar.values())[0], {r["ID"]: r for r in list(en.values())[0]}
        for r in ar_rows:
            e = en_rows.get(r["ID"], {})
            # The book wraps each dhikr in (( )) and its translation in ( ); not needed on a wallpaper.
            text = clean(r["ARABIC_TEXT"]).removeprefix("((").removesuffix(".").removesuffix("))").strip()
            if len(text) > 420:  # too long to read on a wallpaper; still in the book
                continue
            kind = {"istighfar": "istighfar", "durood": "durood", "dhikr": "dhikr"}.get(slot, "dua")
            items.append({"kind": kind, "slot": slot, "ar": text, "en": clean(e.get("TRANSLATED_TEXT", "")).split("[In the evening:]")[0].strip().removeprefix("(").removesuffix(")"), "bn": "",
                          "translit": clean(e.get("LANGUAGE_ARABIC_TRANSLATED_TEXT", "")), "repeat": r.get("REPEAT", 1),
                          "ref": f"Hisn al-Muslim {r['ID']}"})
    return items


def main():
    q, h, a = quran(), hadith(), adhkar()
    lib = {
        "sources": {
            "quran": "holy-quran (MIT): Hafs text; English: Sahih International; Bangla translation",
            "hadith": "hadith-api by Fawaz Ahmed (public domain), with gradings",
            "adhkar": "Hisn al-Muslim by Sa'id ibn Wahf al-Qahtani (hisnmuslim.com)",
        },
        "items": q + h + a,
    }
    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text(json.dumps(lib, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(f"quran {len(q)} · hadith {len(h)} · adhkar {len(a)} → {OUT} ({OUT.stat().st_size // 1024} KB)")
    if "--show" in sys.argv:
        for it in lib["items"]:
            print(f"\n[{it['kind']}] {it['ref']} {it.get('grade', '')}\n  {it['en'][:220]}")


if __name__ == "__main__":
    main()
