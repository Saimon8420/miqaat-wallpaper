"""Exports the calligraphy scenes from scenes.js to assets/calligraphy (one SVG each + scenes.json).

    python tools/calligraphy/export.py

Runs scenes.js in headless Microsoft Edge, so no Node is needed.
"""

import html
import json
import pathlib
import re
import subprocess
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent.parent / "assets" / "calligraphy"
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"


def main():
    with tempfile.TemporaryDirectory() as prof:
        dom = subprocess.run(
            [EDGE, "--headless=new", "--disable-gpu", f"--user-data-dir={prof}", "--allow-file-access-from-files",
             "--virtual-time-budget=5000", "--dump-dom", (HERE / "export.html").as_uri()],
            capture_output=True, text=True, encoding="utf-8", check=True,
        ).stdout
    data = json.loads(html.unescape(re.search(r'<pre id="out">(.*?)</pre>', dom, re.S).group(1)))
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.svg"):
        old.unlink()
    meta = []
    for d in data:
        svg = re.sub(r">\s+<", "><", d.pop("svg").strip())
        # Paper grain (an feTurbulence filter over the whole picture) is slow to render, so it is
        # taken out here and the app adds the same grain with a quick pixel pass instead.
        d["grain"] = 0.0
        for fid, op in re.findall(r'<filter id="([^"]+)"><feTurbulence type="fractalNoise"[^>]*/><feColorMatrix values="[^"]*? ([\d.]+) 0"/></filter>', svg):
            svg = re.sub(rf'<filter id="{fid}">.*?</filter>', "", svg)
            svg = re.sub(rf'<rect[^>]*filter="url\(#{fid}\)"[^>]*/>', "", svg)
            d["grain"] = max(d["grain"], float(op))
        (OUT / f"{d['id']}.svg").write_text(svg, encoding="utf-8")
        meta.append(d)
    (OUT / "scenes.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
    total = sum(f.stat().st_size for f in OUT.glob("*.svg"))
    print(f"{len(meta)} scenes, {total // 1024} KB of SVG")


if __name__ == "__main__":
    main()
