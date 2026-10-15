//! Picks the reminder to show: the right kind for the time of day, from shuffled decks.
//!
//! Every pool (morning adhkar, Quran, hadith, …) is its own deck, shuffled once and walked
//! through, so nothing repeats until the whole deck has been shown.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone)]
pub struct Item {
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub slot: String,
    pub ar: String,
    pub en: String,
    #[serde(default)]
    pub bn: String,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub grade: String,
    #[serde(default)]
    pub long: bool,
}

#[derive(Deserialize)]
struct Library {
    items: Vec<Item>,
}

/// Where in the day we are, as far as reminders care. Minutes are the city's local time.
pub fn moment(now: u32, t: &[u32; 6], ramadan: bool, hijri_day: u32, hijri_month: u32) -> Option<&'static str> {
    let [fajr, sunrise, _dhuhr, asr, maghrib, isha] = *t;
    let after = |p: u32| now >= p + 5 && now < p + 25;
    if ramadan && now + 30 >= maghrib && now < maghrib + 5 {
        return Some("iftar");
    }
    if [fajr, t[2], asr, maghrib, isha].iter().any(|&p| after(p)) {
        return Some("after_salah");
    }
    if hijri_month == 12 && hijri_day == 9 && now >= sunrise && now < maghrib {
        return Some("arafah");
    }
    if hijri_day == 1 && now >= maghrib && now < isha + 60 {
        return Some("new_moon");
    }
    if now + 60 >= fajr && now < fajr {
        return Some("waking");
    }
    if now >= fajr + 25 && now < sunrise + 90 {
        return Some("morning");
    }
    if now >= asr + 25 && now < maghrib {
        return Some("evening");
    }
    if now >= isha + 60 || now + 60 < fajr {
        return Some("sleep");
    }
    None
}

/// Removes Arabic diacritics so words can be compared.
fn bare(s: &str) -> String {
    s.chars().filter(|c| !matches!(*c as u32, 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED)).collect()
}

pub struct Picker {
    items: Vec<Item>,
    decks: HashMap<String, (Vec<usize>, usize)>,
    seed: u64,
    turn: usize,
    /// Where to keep the decks between runs (None in tests and snapshots).
    save_to: Option<std::path::PathBuf>,
}

/// A deck as saved on disk: the order by reference, and how far through it we are.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct SavedDecks {
    turn: usize,
    decks: HashMap<String, (Vec<String>, usize)>,
}

impl Picker {
    pub fn new() -> Self {
        let lib: Library = serde_json::from_str(crate::assets::CONTENT).unwrap_or(Library { items: vec![] });
        let seed = crate::times::utc_seconds() as u64 | 1;
        Picker { items: lib.items, decks: HashMap::new(), seed, turn: 0, save_to: None }
    }

    /// Like `new`, but continues the decks saved in `path` (and keeps saving there).
    pub fn saved(path: std::path::PathBuf) -> Self {
        let mut p = Picker::new();
        if let Some(saved) = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<SavedDecks>(&s).ok()) {
            p.turn = saved.turn;
            let by_ref: HashMap<&str, usize> = p.items.iter().enumerate().map(|(i, it)| (it.reference.as_str(), i)).collect();
            for (key, (refs, pos)) in saved.decks {
                let idx: Vec<usize> = refs.iter().filter_map(|r| by_ref.get(r.as_str()).copied()).collect();
                // a deck from an older library (items added or removed) is simply reshuffled
                if idx.len() == refs.len() && !idx.is_empty() {
                    p.decks.insert(key, (idx, pos));
                }
            }
        }
        p.save_to = Some(path);
        p
    }

    fn save(&self) {
        let Some(path) = &self.save_to else { return };
        let decks = self.decks.iter().map(|(k, (idx, pos))| (k.clone(), (idx.iter().map(|&i| self.items[i].reference.clone()).collect(), *pos))).collect();
        if let Ok(s) = serde_json::to_string(&SavedDecks { turn: self.turn, decks }) {
            let _ = std::fs::write(path, s);
        }
    }

    fn rand(&mut self) -> u64 {
        // xorshift64
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    /// Next item from the deck `key`, building and shuffling it on first use.
    fn draw(&mut self, key: &str, keep: impl Fn(&Item) -> bool) -> Option<Item> {
        let fits = self.decks.get(key).map(|(idx, _)| idx.iter().all(|&i| keep(&self.items[i]))).unwrap_or(false);
        if !fits {
            let mut idx: Vec<usize> = (0..self.items.len()).filter(|&i| keep(&self.items[i])).collect();
            for i in (1..idx.len()).rev() {
                let j = (self.rand() % (i as u64 + 1)) as usize;
                idx.swap(i, j);
            }
            self.decks.insert(key.to_string(), (idx, 0));
        }
        let (len, pos) = self.decks.get(key).map(|(d, p)| (d.len(), *p))?;
        if len == 0 {
            return None;
        }
        if pos >= len {
            // the whole deck has been shown: shuffle a new round
            let swaps: Vec<(usize, usize)> = (1..len).rev().map(|i| (i, (self.rand() % (i as u64 + 1)) as usize)).collect();
            let (deck, p) = self.decks.get_mut(key)?;
            for (i, j) in swaps {
                deck.swap(i, j);
            }
            *p = 0;
        }
        let (deck, p) = self.decks.get_mut(key)?;
        let item = self.items[deck[*p]].clone();
        *p += 1;
        self.save();
        Some(item)
    }

    /// A reminder for this moment: (label key, item). Time-of-day adhkar come first, then
    /// the enabled kinds take turns.
    pub fn pick(&mut self, moment: Option<&str>, kinds: &[String], friday: bool) -> Option<(String, Item)> {
        if let Some(m) = moment {
            let slot = match m {
                "morning" | "evening" => "morning_evening",
                s => s,
            };
            let evening = m == "evening";
            let got = self.draw(m, |it| {
                it.slot == slot && !(evening && (bare(&it.ar).contains("أصبح") || bare(&it.ar).contains("اصبح") || it.en.to_lowercase().contains("morning")))
            });
            if let Some(it) = got {
                return Some((m.to_string(), it));
            }
        }
        let mut kinds: Vec<&str> = kinds.iter().map(|s| s.as_str()).collect();
        if kinds.is_empty() {
            return None;
        }
        // On Fridays durood gets every other turn.
        if friday && kinds.contains(&"durood") && self.turn % 2 == 0 {
            kinds = vec!["durood"];
        }
        let kind = kinds[self.turn % kinds.len()].to_string();
        self.turn += 1;
        let it = match kind.as_str() {
            "quran" => self.draw("quran", |it| it.kind == "quran"),
            "hadith" => self.draw("hadith", |it| it.kind == "hadith" && !it.long),
            k => {
                let k = k.to_string();
                self.draw(&k.clone(), move |it| it.kind == k && it.slot != "morning_evening" && it.slot != "sleep")
            }
        }?;
        Some((kind, it))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: [u32; 6] = [280, 355, 710, 950, 1065, 1140]; // 4:40 5:55 11:50 15:50 17:45 19:00

    #[test]
    fn moments_follow_the_day() {
        assert_eq!(moment(330, &T, false, 3, 5), Some("morning"));
        assert_eq!(moment(720, &T, false, 3, 5), Some("after_salah"));
        assert_eq!(moment(800, &T, false, 3, 5), None);
        assert_eq!(moment(1000, &T, false, 3, 5), Some("evening"));
        assert_eq!(moment(1300, &T, false, 3, 5), Some("sleep"));
        assert_eq!(moment(250, &T, false, 3, 5), Some("waking"));
        assert_eq!(moment(1050, &T, true, 3, 9), Some("iftar"));
    }

    #[test]
    fn decks_continue_after_a_restart() {
        let path = std::env::temp_dir().join(format!("miqaat-decks-test-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let kinds: Vec<String> = vec!["quran".into()];
        let mut first = Picker::saved(path.clone());
        let a: Vec<String> = (0..3).map(|_| first.pick(None, &kinds, false).unwrap().1.reference).collect();
        // "restart": a new picker reading the same file carries on with the next verses
        let mut second = Picker::saved(path.clone());
        let b: Vec<String> = (0..3).map(|_| second.pick(None, &kinds, false).unwrap().1.reference).collect();
        assert!(a.iter().all(|r| !b.contains(r)), "{a:?} repeated in {b:?}");
        // a full round shows every verse once before any repeats
        let mut fresh = Picker::new();
        let n = fresh.items.iter().filter(|i| i.kind == "quran").count();
        let round: std::collections::HashSet<String> = (0..n).map(|_| fresh.pick(None, &kinds, false).unwrap().1.reference).collect();
        assert_eq!(round.len(), n);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn library_loads_and_decks_cycle() {
        let mut p = Picker::new();
        assert!(p.items.len() > 100);
        let kinds: Vec<String> = ["quran", "hadith"].map(String::from).to_vec();
        let (k1, _) = p.pick(None, &kinds, false).unwrap();
        let (k2, _) = p.pick(None, &kinds, false).unwrap();
        assert_ne!(k1, k2);
        let (m, it) = p.pick(Some("morning"), &kinds, false).unwrap();
        assert_eq!(m, "morning");
        assert_eq!(it.slot, "morning_evening");
    }
}
