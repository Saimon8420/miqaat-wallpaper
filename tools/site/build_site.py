"""Builds the live demo page (docs/index.html, served by GitHub Pages) from tools/site/template.html.

    python tools/site/build_site.py
"""
import io
import json
import pathlib
import subprocess

from PIL import Image

HERE = pathlib.Path(__file__).parent
REPO = HERE.parent.parent
DOCS = REPO / "docs"
DL = "https://github.com/Saimon8420/miqaat-wallpaper/releases/latest/download/MiqaatWallpaper-Setup.exe"
GH = "https://github.com/Saimon8420/miqaat-wallpaper"

s = (HERE / "template.html").read_text(encoding="utf-8")


def ed(a, b):
    global s
    assert a in s, a[:80]
    s = s.replace(a, b, 1)


# scenes as small files next to the page
(DOCS / "scenes").mkdir(parents=True, exist_ok=True)
scenes = json.loads((REPO / "assets/scenes/scenes.json").read_text(encoding="utf-8"))
out = []
for sc in scenes:
    im = Image.open(REPO / f"assets/scenes/{sc['id']}.jpg").convert("RGB").resize((1280, 720), Image.LANCZOS)
    im.save(DOCS / f"scenes/{sc['id']}.jpg", "JPEG", quality=74, optimize=True, progressive=True)
    out.append({"id": sc["id"], "name": sc["name"], "img": f"scenes/{sc['id']}.jpg", "credit": sc["credit"]})
ed("const SCENES={{SCENES}};", "const SCENES=" + json.dumps(out, ensure_ascii=False) + ";")

ed("<title>Proposal · Miqaat Live Wallpaper</title>",
   '<title>Miqaat Wallpaper · a calm prayer-time wallpaper for Windows</title>\n'
   '<meta name="description" content="Free live wallpaper for Windows: prayer times, the next prayer, and one authentic reminder at a time with its source. Written in Rust, about 30 MB of memory.">\n'
   '<link rel="icon" href="icon.png">')
ed('<span class="eyebrow">Proposal · Desktop</span>', '<span class="eyebrow">Free · Windows 10 &amp; 11</span>')
ed("<h1>Miqaat Live Wallpaper.<br>", "<h1>Miqaat Wallpaper.<br>")
ed('Written in Rust to use almost no memory.</p>',
   'Written in Rust; about 30 MB of memory.</p>\n'
   f'  <div class="cta"><a class="dl" href="{DL}">Download for Windows<span>MiqaatWallpaper-Setup.exe · 37 MB</span></a>'
   f'<a class="gh" href="{GH}">Source on GitHub</a><a class="gh" href="#install">How to install</a></div>')
ed("The settings above are live in this preview: city, method, Asr time and Hijri date fetch real times from the Miqaat API.",
   "This page is a live preview of the app: city, method, Asr time and Hijri date fetch real times from the Miqaat API. The real wallpaper draws the same thing natively, behind your desktop icons.")
ed('<div class="cell b4"><div class="core"><span class="tag">Memory</span><div class="big">15–35<small> MB</small></div><p style="margin-top:10px">Target in Rust.',
   '<div class="cell b4"><div class="core"><span class="tag">Memory</span><div class="big">~30<small> MB</small></div><p style="margin-top:10px">Measured on a two-monitor 1080p desktop.')
ed('<div class="me">Rust (chosen)<span>~25 MB</span>', '<div class="me">Rust (this app)<span>~30 MB</span>')
ed("No text will be typed from memory. The Quran comes from the Tanzil verified Uthmani text; hadith only from collections with their grading shown; adhkar from Hisn al-Muslim with references. Days whose special worship is disputed are not presented as worship days. Each dataset's licence is checked before use.",
   "No text was typed from memory: every word is pulled from published datasets by a build script, and only the references were chosen by hand. Hadith come only from collections with their grading shown; adhkar from Hisn al-Muslim with item numbers. Days whose special worship is disputed are not presented as worship days.")
ed("<li><b>Quran:</b> the same source as your al-kitab app: the <i>holy-quran</i> library (MIT), Hafs text with English (Sahih International) and Bangla translations, all 6,236 verses</li>",
   "<li><b>Quran:</b> the <i>holy-quran</i> library (MIT): Hafs text with English (Sahih International) and Bangla translations; 237 short, complete verses chosen for a wallpaper</li>")
ed("<li><b>Hadith:</b> the free <i>hadith-api</i> (public domain): Bukhari, Muslim, Abu Dawud, Tirmidhi, Nasa'i, Ibn Majah and Malik in Arabic, English and Bangla, with gradings. Every hadith we show is cross-checked against sunnah.com</li>",
   "<li><b>Hadith:</b> the free <i>hadith-api</i> (public domain) in Arabic, English and Bangla: 224 hadith from Bukhari, Muslim, Tirmidhi, Abu Dawud, Ibn Majah and an-Nawawi's Forty, each with its grading</li>")
ed("<li><b>Duas, dhikr, istighfar, durood and what to do on each day:</b> our own library, written from the Quran and hadith with a reference on every item, and checked by you before release</li>",
   "<li><b>Duas, dhikr, istighfar and durood:</b> 220 items from Hisn al-Muslim (Fortress of the Muslim), with the book's item number on each</li>")
ed("<li>All of it is packed into one small offline file (about 3–5 MB), so nothing needs the internet</li>",
   "<li>All of it is one 1 MB file built into the app, so reminders never need the internet</li>")


# Calligraphy gallery: real renders from the app itself (needs a release build in target/release).
GALLERY = [("illuminated-page", "Illuminated page"), ("night-over-the-mosque", "Night over the mosque"), ("lanterns", "Lanterns"),
           ("tone-on-tone-allah", "Tone on tone"), ("ramadan-night", "Ramadan night"), ("eid-morning", "Eid morning"),
           ("day-of-arafah", "Day of Arafah"), ("eid-prayer-at-the-eidgah", "Eid al-Adha"), ("takbir-of-eid", "Takbir of Eid")]
EXE = REPO / "target/release/MiqaatWallpaper.exe"
(DOCS / "calligraphy").mkdir(parents=True, exist_ok=True)
for sid, _ in GALLERY:
    png = DOCS / f"calligraphy/{sid}.png"
    if EXE.exists():
        subprocess.run([str(EXE), "--snapshot", str(png), "--size", "1600x900", "--at", "13:12", "--set", f"scene=cal:{sid}",
                        "--set", "rotate=keep", "--set", "seconds=false", "--set", "show_monitor=false", "--set", "lang=en",
                        "--set", "layout=column", "--set", "side=right"], check=True)
        Image.open(png).convert("RGB").resize((960, 540), Image.LANCZOS).save(DOCS / f"calligraphy/{sid}.jpg", "JPEG", quality=80, optimize=True, progressive=True)
        png.unlink()
# Clock designs, rendered the same way.
CLOCKS = [("classic", "Classic"), ("split", "Bold hours"), ("stack", "Stacked"), ("tiles", "Tiles"), ("pill", "Pill"), ("dial", "Dial")]
for cid, _ in CLOCKS:
    png = DOCS / f"calligraphy/clock-{cid}.png"
    if EXE.exists():
        subprocess.run([str(EXE), "--snapshot", str(png), "--size", "1600x900", "--at", "13:12:07", "--set", "scene=wadirum",
                        "--set", "rotate=keep", "--set", "seconds=true", "--set", "show_monitor=false", "--set", "lang=en",
                        "--set", "layout=column", "--set", "side=left", "--set", f"clock_design={cid}", "--set", "info_style=cards"], check=True)
        Image.open(png).convert("RGB").crop((0, 0, 800, 450)).resize((640, 360), Image.LANCZOS).save(DOCS / f"calligraphy/clock-{cid}.jpg", "JPEG", quality=82, optimize=True, progressive=True)
        png.unlink()
clocks = "".join(f'<figure><img src="calligraphy/clock-{cid}.jpg" alt="{name} clock" loading="lazy"><figcaption>{name}</figcaption></figure>' for cid, name in CLOCKS)
gallery = "".join(f'<figure><img src="calligraphy/{sid}.jpg" alt="{name}" loading="lazy"><figcaption>{name}</figcaption></figure>' for sid, name in GALLERY)

ed("""        <li>Turn it off and the wallpaper goes back to updating once a minute</li></ul></div></div>
    </div>""", """        <li>Turn it off and the wallpaper goes back to updating once a minute</li></ul></div></div>
      <div class="cell b6"><div class="core"><span class="tag">Calligraphy</span><h3>13 scenes drawn by the app</h3><ul>
        <li>An illuminated page, a night over an Ottoman mosque, dunes at dawn, lanterns, ink and seal, zellige, an oasis, a moonlit sea and more</li>
        <li>Each with a phrase in bold Kufi lettering, its meaning and its source</li>
        <li>Vector art, sharp on any screen; drawn once when the picture changes, no extra memory</li></ul></div></div>
      <div class="cell b6"><div class="core"><span class="tag">Ramadan &amp; Eid</span><h3>The right picture on the right day</h3><ul>
        <li>Ramadan nights all month, Laylat al-Qadr in the last ten nights</li>
        <li>Eid al-Fitr, the Day of Arafah and Eid al-Adha, by the Hijri date</li>
        <li>Then your own picture comes back by itself</li></ul></div></div>
      <div class="cell b8"><div class="core"><span class="tag">On the wallpaper</span><h3>A little more, when you want it</h3><ul>
        <li>Suhoor and iftar times, with a countdown to iftar while you fast</li>
        <li>The Qibla direction for your city, and the days left until Ramadan and the two Eids</li>
        <li>A quiet notification a few minutes before each prayer, and before Jumu'ah</li>
        <li>69 fonts in Arabic, Bangla and English, and 18 ways to style the clock, date, text and cards</li></ul></div></div>
      <div class="cell b6"><div class="core"><span class="tag">Clock</span><h3>Six designs, or none</h3><ul>
        <li>Classic, bold hours, stacked, flip tiles, a pill with a seconds ring, or an analog dial</li>
        <li>Turn the clock off and the next prayer becomes the headline</li>
        <li>With seconds on, only the seconds change; nothing else moves</li></ul></div></div>
      <div class="cell b6"><div class="core"><span class="tag">At a glance</span><h3>Fasting, next event and Qibla</h3><ul>
        <li>As lines, chips, small cards or one ruled strip</li>
        <li>The iftar card shows how much of the fast is done</li>
        <li>Uses the empty space beside the countdown</li></ul></div></div>
      <div class="cell b4"><div class="core"><span class="tag">Updates</span><h3>Checks once a day</h3><p style="margin-top:12px">A small helper asks GitHub for a new version and closes again. You choose when to install, from the tray menu.</p></div></div>
    </div>""")

# Plan + decisions -> install section
start = s.index('  <section>\n    <span class="eyebrow">Plan</span>')
end = s.index("</section>", start) + len("</section>")
install = f'''  <section id="install">
    <span class="eyebrow">Install</span>
    <h2>One download.<br>No admin, no account.</h2>
    <div class="steps">
      <div><span>01</span><p><b>Download</b> <a href="{DL}">MiqaatWallpaper-Setup.exe</a> (about 37 MB) from the latest release.</p></div>
      <div><span>02</span><p><b>Run it.</b> Windows may show "Windows protected your PC" because the app is new and not code-signed: click <i>More info</i>, then <i>Run anyway</i>.</p></div>
      <div><span>03</span><p><b>Choose Yes</b> to install for your account only. It copies itself to your user folder, adds a Start menu entry and starts with Windows. Choose <i>No</i> to just try it once.</p></div>
      <div><span>04</span><p><b>Set your city.</b> Settings open on first run; later, right-click the moon icon near the clock. Changes show on the wallpaper straight away.</p></div>
      <div><span>05</span><p><b>Uninstall</b> any time from Settings › Apps › Installed apps › Miqaat Wallpaper. Your normal wallpaper comes back.</p></div>
    </div>
    <div class="ask"><div><h3>Free and open source</h3><p>MIT licensed. Photos are from Wikimedia Commons under their own licences, fonts under the SIL Open Font Licence; every credit is in the repository.</p></div><a class="dl" href="{DL}">Download<span>Windows 10 &amp; 11</span></a></div>
  </section>'''
gallery_section = f'''  <section id="calligraphy">
    <span class="eyebrow">Calligraphy</span>
    <h2>Scenes the app draws itself.</h2>
    <div class="gal">{gallery}</div>
  </section>
  <section id="clocks">
    <span class="eyebrow">Clock</span>
    <h2>Six clocks, or none at all.</h2>
    <div class="gal">{clocks}</div>
  </section>
'''
s = s[:start] + gallery_section + install + s[end:]

ed("</style>", """.cta{display:flex;flex-wrap:wrap;gap:12px;align-items:center;margin-top:28px}
.dl{display:inline-flex;flex-direction:column;gap:2px;padding:14px 26px;border-radius:99px;background:var(--ink);color:#fff;text-decoration:none;font-weight:500;font-size:15px;transition:transform .5s var(--ease)}
.dl span{font-size:11.5px;font-weight:400;opacity:.65}
.dl:hover{transform:translateY(-1px)}.dl:active{transform:scale(.98)}
.gh{padding:12px 18px;border-radius:99px;color:var(--ink);text-decoration:none;font-size:14.5px;font-weight:500;background:rgba(22,20,15,.05)}
.gh:hover{background:rgba(22,20,15,.09)}
.steps a{color:var(--pri)}
.ask{display:flex;gap:24px;align-items:center;justify-content:space-between;flex-wrap:wrap}
.gal{display:grid;grid-template-columns:repeat(auto-fill,minmax(340px,1fr));gap:14px;margin-top:30px}
.gal figure{margin:0;padding:6px;border-radius:20px;background:rgba(22,20,15,.035);box-shadow:inset 0 0 0 1px rgba(22,20,15,.05)}
.gal img{display:block;width:100%;aspect-ratio:16/9;object-fit:cover;border-radius:14px}
.gal figcaption{font-size:13.5px;color:var(--ink2);padding:8px 6px 2px}
</style>""")

(DOCS / "index.html").write_text(s, encoding="utf-8")
Image.open(REPO / "assets/icon.png").resize((64, 64), Image.LANCZOS).save(DOCS / "icon.png")
print("ok", len(s) // 1024, "KB")
