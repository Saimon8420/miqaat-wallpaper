//! The wallpaper's state: settings, the month of prayer times, the current reminder, and
//! turning all of it into a `Frame` to draw.

use crate::content::{Item, Picker};
use crate::hijri::{self, Hijri};
use crate::i18n::{self, Lang};
use crate::render::{Frame, Photo, Renderer, Style};
use crate::settings::Settings;
use crate::sysmon::Monitor;
use crate::times::{self, Day, Month};
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};

pub struct App {
    pub s: Settings,
    pub month: Option<Month>,
    pub fetching: bool,
    pub failed: bool,
    picker: Picker,
    pub reminder: Option<(String, Item)>,
    reminder_at: i64,
    last_moment: Option<&'static str>,
    pub renderer: Option<Renderer>,
    look_key: String,
    pub monitors: Vec<RECT>,
    pub primary: usize,
    pub sysmon: Option<Monitor>,
    /// Paused from the tray menu.
    pub paused: bool,
    /// A full-screen game or video is running.
    pub busy: bool,
    pub light_key: [i32; 5],
    pub scene_key: Vec<usize>,
    pub scene_offset: usize,
    pub notified: Vec<(i64, usize)>,
    pub user_wallpaper: String,
    /// Pretend it is this second of the day (snapshots of other hours).
    pub fake_now: Option<u32>,
}

/// The PC's own offset from UTC in minutes (used until a month of times is loaded).
pub fn pc_offset() -> i32 {
    unsafe {
        let (l, u) = (GetLocalTime(), GetSystemTime());
        let lm = times::days_from_civil(l.wYear as i32, l.wMonth as u32, l.wDay as u32) * 1440 + l.wHour as i64 * 60 + l.wMinute as i64;
        let um = times::days_from_civil(u.wYear as i32, u.wMonth as u32, u.wDay as u32) * 1440 + u.wHour as i64 * 60 + u.wMinute as i64;
        (lm - um) as i32
    }
}

pub fn on_battery() -> bool {
    let mut p = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut p).is_ok() && p.ACLineStatus == 0 }
}

fn date_text(days: i64) -> String {
    let (y, m, d) = times::civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

const WEEKDAYS: [[&str; 7]; 3] = [
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    ["রবিবার", "সোমবার", "মঙ্গলবার", "বুধবার", "বৃহস্পতিবার", "শুক্রবার", "শনিবার"],
    ["الأحد", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت"],
];

impl App {
    pub fn new(s: Settings) -> Self {
        App {
            s,
            month: None,
            fetching: false,
            failed: false,
            picker: Picker::new(),
            reminder: None,
            reminder_at: 0,
            last_moment: None,
            renderer: None,
            look_key: String::new(),
            monitors: vec![],
            primary: 0,
            sysmon: None,
            paused: false,
            busy: false,
            light_key: [0; 5],
            scene_key: vec![],
            scene_offset: 0,
            notified: vec![],
            user_wallpaper: String::new(),
            fake_now: None,
        }
    }

    pub fn lang(&self) -> Lang {
        Lang::from(&self.s.lang)
    }

    pub fn offset(&self) -> i32 {
        self.month.as_ref().map(|m| m.offset).unwrap_or_else(pc_offset)
    }

    /// City-local (days since 1970, second of day).
    pub fn now(&self) -> (i64, u32) {
        let (days, sec) = times::local_now(self.offset());
        (days, self.fake_now.unwrap_or(sec))
    }

    /// The month key the city's current date needs.
    pub fn wanted_key(&self) -> String {
        let (days, _) = self.now();
        let (y, m, _) = times::civil_from_days(days);
        times::month_key(self.s.lat, self.s.lng, &self.s.method, &self.s.madhab, y, m)
    }

    pub fn day(&self, days: i64) -> Option<&Day> {
        let d = date_text(days);
        self.month.as_ref()?.days.iter().find(|x| x.date == d)
    }

    /// Today's Hijri date with the user's adjustment.
    pub fn hijri(&self, days: i64) -> Option<Hijri> {
        let adj = self.s.hijri_adjust;
        if let Some(d) = self.day(days + adj as i64) {
            return hijri::parse(&d.hijri);
        }
        Some(hijri::parse(&self.day(days)?.hijri)?.shift(adj))
    }

    fn battery_saving(&self) -> bool {
        self.s.battery_saver && on_battery()
    }

    // ---------- pictures ----------

    /// Which built-in scene each monitor shows (index into SCENES).
    pub fn scenes_now(&self) -> Vec<usize> {
        let n = crate::assets::SCENES.len();
        let (days, sec) = self.now();
        let base = match self.s.rotate.as_str() {
            "daily" => days.rem_euclid(n as i64) as usize,
            "prayer" => {
                let part = self.day(days).map(|d| d.minutes.iter().filter(|&&m| m <= sec / 60).count()).unwrap_or(0);
                ((days * 7 + part as i64).rem_euclid(n as i64)) as usize
            }
            _ => crate::assets::SCENES.iter().position(|x| x.id == self.s.scene).unwrap_or(0),
        };
        (0..self.monitors.len().max(1)).map(|k| (base + self.scene_offset + if self.s.monitors == "different" { k } else { 0 }) % n).collect()
    }

    fn photos(&self, scenes: &[usize]) -> Vec<(Photo, u32, u32)> {
        let custom = self.s.scene == "custom" && std::path::Path::new(&self.s.custom_photo).exists();
        self.monitors
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let size = ((m.right - m.left).max(1) as u32, (m.bottom - m.top).max(1) as u32);
                let src = if self.s.monitors == "main" && k != self.primary && std::path::Path::new(&self.user_wallpaper).exists() {
                    Photo::File(self.user_wallpaper.clone())
                } else if custom {
                    Photo::File(self.s.custom_photo.clone())
                } else {
                    Photo::Embedded(crate::assets::SCENES[scenes[k]].jpg)
                };
                (src, size.0, size.1)
            })
            .collect()
    }

    /// (Re)builds the renderer when the pictures or fonts changed. Returns true if it did.
    pub fn ensure_renderer(&mut self) -> bool {
        let scenes = self.scenes_now();
        let key = format!("{}|{:?}|{:?}", self.s.look_key(), scenes, self.monitors.iter().map(|r| (r.left, r.top, r.right, r.bottom)).collect::<Vec<_>>());
        if self.renderer.is_some() && key == self.look_key {
            return false;
        }
        self.renderer = None; // free the old pictures first
        let photos = self.photos(&scenes);
        self.renderer = Renderer::new(&photos, &self.s.effect, self.s.grain, self.s.behind == "glass", &self.s.font_ar, &self.s.font_bn, &self.s.font_en)
            .map_err(|e| crate::log(&format!("renderer: {e}")))
            .ok();
        if self.renderer.is_none() && self.s.scene == "custom" {
            // A broken custom photo shouldn't leave a black desktop.
            let fallback: Vec<_> = photos.iter().enumerate().map(|(k, p)| (Photo::Embedded(crate::assets::SCENES[scenes[k]].jpg), p.1, p.2)).collect();
            self.renderer = Renderer::new(&fallback, &self.s.effect, self.s.grain, self.s.behind == "glass", &self.s.font_ar, &self.s.font_bn, &self.s.font_en).ok();
        }
        self.look_key = key;
        self.scene_key = scenes;
        true
    }

    // ---------- reminders ----------

    /// Picks a new reminder when the moment of day changes or the interval has passed.
    /// Returns true when the reminder changed.
    pub fn rotate_reminder(&mut self, force: bool) -> bool {
        let (days, sec) = self.now();
        let now_min = sec / 60;
        let utc = times::utc_seconds();
        let (moment, friday) = match self.day(days) {
            Some(d) if self.s.follow_time => {
                let h = self.hijri(days);
                let (hd, hm) = h.map(|h| (h.day, h.month)).unwrap_or((0, 0));
                (crate::content::moment(now_min, &d.minutes, hm == 9, hd, hm), times::weekday(days) == 5)
            }
            _ => (None, times::weekday(days) == 5),
        };
        let due = utc - self.reminder_at >= self.s.interval.max(1) as i64 * 60;
        if !(force || self.reminder.is_none() || due || moment != self.last_moment) {
            return false;
        }
        self.last_moment = moment;
        self.reminder_at = utc;
        self.reminder = self.picker.pick(moment, &self.s.kinds, friday);
        true
    }

    // ---------- the frame ----------

    pub fn style(&self) -> Style {
        let l = self.lang();
        Style {
            layout: self.s.layout.clone(),
            side_left: self.s.side == "left",
            behind: self.s.behind.clone(),
            shade: self.s.shade as f32 / 100.0,
            accent: crate::render::accent(&self.s.accent),
            scale: self.s.text_size.clamp(70, 140) as f32 / 100.0,
            ar_scale: self.s.ar_size.clamp(70, 150) as f32 / 100.0,
            clock_style: self.s.clock_style.clone(),
            rtl_labels: l == Lang::Ar,
            bn_labels: l == Lang::Bn,
            show_next: self.s.show_next,
            show_reminder: self.s.show_reminder,
            show_special: self.s.show_special,
            show_times: self.s.show_times,
        }
    }

    fn fmt_time(&self, min: u32) -> (String, String) {
        let l = self.lang();
        let (h, m) = ((min / 60) % 24, min % 60);
        if self.s.clock24 {
            (i18n::digits(l, self.s.arabic_digits, &format!("{h:02}:{m:02}")), String::new())
        } else {
            let h12 = if h % 12 == 0 { 12 } else { h % 12 };
            (i18n::digits(l, self.s.arabic_digits, &format!("{h12}:{m:02}")), i18n::ampm(l, h >= 12).to_string())
        }
    }

    pub fn frame(&self) -> Frame {
        let l = self.lang();
        let (days, sec) = self.now();
        let now = sec / 60;
        let (clock, ampm) = self.fmt_time(now);
        let seconds = self.s.seconds.then(|| i18n::digits(l, self.s.arabic_digits, &format!(":{:02}", sec % 60)));
        let place = self.s.place.split(',').next().unwrap_or("").trim().to_string();
        let wd = WEEKDAYS[match l {
            Lang::En => 0,
            Lang::Bn => 1,
            Lang::Ar => 2,
        }][times::weekday(days) as usize];
        let h = self.hijri(days);
        let mut meta = vec![wd.to_string()];
        if let Some(h) = h {
            meta.push(h.text(l));
        }
        if !place.is_empty() {
            meta.push(place);
        }

        let (next, progress, rows);
        let mut minutes = [300, 360, 720, 960, 1080, 1170];
        if let Some(d) = self.day(days) {
            minutes = d.minutes;
            let t = &d.minutes;
            let (ni, at) = match t.iter().position(|&p| p > now) {
                Some(i) => (i, t[i]),
                None => (0, self.day(days + 1).map(|x| x.minutes[0]).unwrap_or(t[0]) + 1440),
            };
            let prev = if ni == 0 { t[5] as i64 - if now >= t[5] { 0 } else { 1440 } } else { t[ni - 1] as i64 };
            let span = (at as i64 - prev).max(1) as f32;
            progress = (now as i64 - prev) as f32 / span;
            next = i18n::digits(l, false, &i18n::next_in(l, i18n::prayer(l, ni), at - now));
            let cur = (0..6).filter(|&i| i != 1 && t[i] <= now).last().unwrap_or(5);
            rows = (0..6).map(|i| (i18n::prayer(l, i).to_string(), self.fmt_time(t[i]).0, i == cur)).collect();
        } else {
            (progress, rows) = (0.0, vec![]);
            next = i18n::phrase(l, if self.failed { "offline" } else { "loading" }).to_string();
        }

        let special = if self.s.special_days {
            h.and_then(|h| hijri::special(h, times::weekday(days), l)).map(|sp| (sp.text, sp.reference.to_string()))
        } else {
            None
        };

        let (mut kind, mut arabic, mut meaning, mut source, mut meaning_bn) = (String::new(), String::new(), String::new(), String::new(), false);
        if let Some((k, it)) = &self.reminder {
            kind = i18n::kind(l, k).to_string();
            if !(it.kind == "hadith" && it.ar.chars().count() > 280) {
                arabic = it.ar.clone();
            }
            meaning = match self.s.meaning.as_str() {
                "none" => String::new(),
                "bn" if !it.bn.is_empty() => {
                    meaning_bn = true;
                    it.bn.clone()
                }
                _ => it.en.clone(),
            };
            source = if it.grade.is_empty() { it.reference.clone() } else { format!("{} · {}", it.reference, it.grade) };
        }

        let saving = self.battery_saving();
        let light = crate::light::at(now, &minutes, self.s.night_dark.min(100) as f32 / 100.0, self.s.light_follows);
        let zoom = if self.s.drift && !saving {
            let t = (times::utc_seconds() / 60) as f32 % 40.0 / 40.0;
            1.0 + 0.04 * (0.5 - 0.5 * (t * std::f32::consts::TAU).cos())
        } else {
            1.0
        };
        let monitor = match (&self.sysmon, self.s.show_monitor && !saving) {
            (Some(m), true) => Some((
                m.now.clone(),
                [0, 1, 2, 3, 4].map(|i| m.history[i].iter().copied().collect::<Vec<f32>>()),
                ["CPU", "MEMORY", "GPU", "DISK", "NETWORK"].map(String::from),
            )),
            _ => None,
        };

        Frame {
            clock,
            ampm,
            seconds,
            // A dot would read as the Arabic zero (٠), so Arabic uses its own comma.
            meta: meta.join(if l == Lang::Ar { "، " } else { " · " }),
            next,
            progress,
            special,
            kind,
            arabic,
            meaning,
            meaning_bn,
            source,
            times: rows,
            light,
            zoom,
            monitor,
        }
    }

    /// Part of the primary monitor that changes every minute (client coordinates).
    pub fn panel_rect(&self) -> Option<RECT> {
        let p = *self.monitors.get(self.primary)?;
        let (w, h) = (p.right - p.left, p.bottom - p.top);
        Some(match self.s.layout.as_str() {
            "centre" => p,
            "bar" => RECT { top: p.bottom - h * 45 / 100, ..p },
            "corner" if self.s.side == "left" => RECT { right: p.left + w * 45 / 100, top: p.top + h * 25 / 100, ..p },
            "corner" => RECT { left: p.left + w * 55 / 100, top: p.top + h * 25 / 100, ..p },
            _ if self.s.side == "left" => RECT { right: p.left + w * 62 / 100, ..p },
            _ => RECT { left: p.left + w * 38 / 100, ..p },
        })
    }

    /// Prayer-time and day-before notifications due this minute: (title, body).
    pub fn due_notifications(&mut self) -> Vec<(String, String)> {
        let mut out = vec![];
        if !self.s.notify {
            return out;
        }
        let l = self.lang();
        let (days, sec) = self.now();
        let now = sec / 60;
        let Some(d) = self.day(days).cloned() else { return out };
        for i in [0usize, 2, 3, 4, 5] {
            if d.minutes[i] == now && !self.notified.contains(&(days, i)) {
                self.notified.push((days, i));
                let (t, ap) = self.fmt_time(d.minutes[i]);
                out.push((format!("{} {}", i18n::phrase(l, "time_for"), i18n::prayer(l, i)), format!("{t} {ap} · {}", self.s.place)));
            }
        }
        // The evening before a fasting day, a heads-up at Maghrib.
        if self.s.special_days && d.minutes[4] == now && !self.notified.contains(&(days, 9)) {
            self.notified.push((days, 9));
            if let Some(h) = self.hijri(days + 1).or_else(|| self.hijri(days).map(|h| h.shift(1))) {
                if let Some(sp) = hijri::special(h, times::weekday(days + 1), l) {
                    if sp.heads_up {
                        out.push((i18n::phrase(l, "tomorrow").to_string(), format!("{}\n{}", sp.text, sp.reference)));
                    }
                }
            }
        }
        self.notified.retain(|(d, _)| *d >= days - 1);
        out
    }
}
