// Reads verses from the holy-quran library. Usage: node quran.mjs "2:152-152,3:8-8"
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const q = require("holy-quran");
import { surahNamesEn } from "quran-meta/i18n";

const out = {};
for (const ref of process.argv[2].split(",")) {
  const [s, range] = ref.split(":");
  const [a, b] = range.split("-").map(Number);
  const sura = Number(s);
  const verses = [];
  for (let v = a; v <= b; v++) {
    verses.push({
      ar: q.HolyQuranHafsVersion[sura - 1].verses[v - 1],
      en: q.QuranEnglish[sura - 1][v - 1],
      bn: q.QuranBengali[sura - 1][v - 1],
    });
  }
  out[ref] = { surahAr: q.HolyQuranHafsVersion[sura - 1].name, surahEn: surahNamesEn[sura][0], verses };
}
process.stdout.write(JSON.stringify(out));
