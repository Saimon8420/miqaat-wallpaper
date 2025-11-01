//! The settings window: plain Win32 controls, six sections, every change applied live.
//!
//! Rows are described once in `rows()` and read/written through the settings' JSON form, so
//! adding an option is one line here plus a field in `Settings`.

use crate::settings::Settings;
use serde_json::Value;
use std::cell::RefCell;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW};
use windows::Win32::UI::Controls::*;
use windows::Win32::System::SystemServices::{SS_ENDELLIPSIS, SS_NOPREFIX};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

const SECTIONS: [&str; 6] = ["Location & times", "Reminders", "Appearance", "Design", "Fonts & language", "System"];
const WM_CITIES: u32 = WM_APP + 21;
const TIMER_SLIDER: usize = 7;
const ID_NAV: i32 = 100;
const TBM_GETPOS: u32 = WM_USER;
const ID_SEARCH: i32 = 1; // IDOK, so Enter in the city box searches

enum Kind {
    Choice(Vec<(String, String)>),
    Toggle,
    Slider(i32, i32),
    /// Several bool fields as checkboxes: (field, label).
    Checks(Vec<(&'static str, &'static str)>),
    /// The reminder kinds list.
    Kinds,
    City,
    Results,
    PhotoButton,
    About,
}

struct Row {
    sec: usize,
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    kind: Kind,
}

fn opts(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn rows() -> Vec<Row> {
    let r = |sec, title, hint, key, kind| Row { sec, title, hint, key, kind };
    let fonts = |list: &[crate::assets::Font]| list.iter().map(|f| (f.name.to_string(), f.name.to_string())).collect();
    let mut scenes: Vec<(String, String)> = crate::assets::SCENES.iter().map(|s| (s.id.to_string(), s.name.to_string())).collect();
    scenes.push(("custom".into(), "My own photo".into()));
    vec![
        r(0, "City", "", "place", Kind::City),
        r(0, "Pick a result", "Search above, then choose your city here", "place", Kind::Results),
        r(
            0,
            "Calculation method",
            "Which authority's angles to use",
            "method",
            Kind::Choice(opts(&[
                ("Karachi", "University of Islamic Sciences, Karachi"),
                ("MuslimWorldLeague", "Muslim World League"),
                ("Egyptian", "Egyptian General Authority of Survey"),
                ("UmmAlQura", "Umm Al-Qura University, Makkah"),
                ("Dubai", "Dubai"),
                ("MoonsightingCommittee", "Moonsighting Committee"),
                ("NorthAmerica", "Islamic Society of North America"),
                ("Kuwait", "Kuwait"),
                ("Qatar", "Qatar"),
                ("Singapore", "Majlis Ugama Islam Singapura"),
                ("Turkey", "Diyanet, Turkey"),
                ("Tehran", "Institute of Geophysics, Tehran"),
            ])),
        ),
        r(0, "Asr time", "Shafi'i, Maliki, Hanbali or Hanafi", "madhab", Kind::Choice(opts(&[("shafi", "Standard"), ("hanafi", "Hanafi")]))),
        r(
            0,
            "Hijri date",
            "Moon sighting differs by country; in Bangladesh it is often −1",
            "hijri_adjust",
            Kind::Choice(opts(&[("-2", "−2 days"), ("-1", "−1 day"), ("0", "As calculated"), ("1", "+1 day"), ("2", "+2 days")])),
        ),
        r(0, "Clock", "How times are written", "clock24", Kind::Choice(opts(&[("false", "12-hour"), ("true", "24-hour")]))),
        r(0, "Notify at prayer times", "A small Windows notification, no sound", "notify", Kind::Toggle),
        r(1, "Show", "Pick the kinds of reminders", "kinds", Kind::Kinds),
        r(
            1,
            "Change every",
            "A new reminder fades in",
            "interval",
            Kind::Choice(opts(&[("5", "5 minutes"), ("10", "10 minutes"), ("15", "15 minutes"), ("30", "30 minutes"), ("60", "1 hour")])),
        ),
        r(1, "Follow the time of day", "Morning and evening adhkar, adhkar after salah, before sleep", "follow_time", Kind::Toggle),
        r(1, "Special days", "White Days, Monday/Thursday fasts, Ashura, Arafah, Ramadan…", "special_days", Kind::Toggle),
        r(1, "Meaning", "Shown under the Arabic", "meaning", Kind::Choice(opts(&[("en", "English"), ("bn", "বাংলা"), ("none", "Arabic only")]))),
        r(2, "Picture", "One of 20 places, or your own photo", "scene", Kind::Choice(scenes)),
        r(
            2,
            "Change picture",
            "Rotate through the places",
            "rotate",
            Kind::Choice(opts(&[("keep", "Keep this picture"), ("daily", "New picture every day"), ("prayer", "New picture each prayer")])),
        ),
        r(2, "Your own photo", "Any JPG or PNG on this PC", "custom_photo", Kind::PhotoButton),
        r(2, "Panel shade", "How dark the area behind the text is", "shade", Kind::Slider(0, 100)),
        r(2, "Side", "Put the text away from your icons", "side", Kind::Choice(opts(&[("right", "Right"), ("left", "Left")]))),
        r(
            2,
            "Show on wallpaper",
            "",
            "",
            Kind::Checks(vec![("show_next", "Next prayer"), ("show_reminder", "Reminder"), ("show_special", "Special day"), ("show_times", "Prayer row"), ("show_monitor", "System monitor")]),
        ),
        r(2, "Light follows the sky", "Rose at Fajr, gold at sunrise, warm at Asr, orange at Maghrib, blue at Isha", "light_follows", Kind::Toggle),
        r(2, "Night darkness", "How dark the picture gets after Isha", "night_dark", Kind::Slider(0, 100)),
        r(
            3,
            "Layout",
            "How the information sits on the picture",
            "layout",
            Kind::Choice(opts(&[("column", "Column"), ("centre", "Centre"), ("corner", "Corner"), ("bar", "Bottom bar")])),
        ),
        r(3, "Clock style", "", "clock_style", Kind::Choice(opts(&[("thin", "Thin"), ("bold", "Bold"), ("serif", "Serif")]))),
        r(3, "Arabic digits", "Arabic-Indic digits for the clock and times", "arabic_digits", Kind::Toggle),
        r(3, "Show seconds", "Redraws only the clock, once a second", "seconds", Kind::Toggle),
        r(
            3,
            "Accent colour",
            "Headings, the progress line and the current prayer",
            "accent",
            Kind::Choice(opts(&[("gold", "Gold"), ("emerald", "Emerald"), ("sky", "Sky"), ("rose", "Rose"), ("pearl", "Pearl")])),
        ),
        r(3, "Text size", "", "text_size", Kind::Slider(80, 130)),
        r(3, "Behind the text", "", "behind", Kind::Choice(opts(&[("shade", "Soft shade"), ("glass", "Glass card"), ("none", "Nothing")]))),
        r(3, "Slow camera drift", "A gentle zoom, redrawn once a minute", "drift", Kind::Toggle),
        r(3, "Film grain", "", "grain", Kind::Toggle),
        r(
            3,
            "Picture effect",
            "",
            "effect",
            Kind::Choice(opts(&[("none", "Natural"), ("warm", "Warm"), ("cool", "Cool"), ("mono", "Black & white"), ("blur", "Soft blur")])),
        ),
        r(4, "Wallpaper language", "Labels and prayer names", "lang", Kind::Choice(opts(&[("en", "English"), ("bn", "বাংলা"), ("ar", "العربية")]))),
        r(4, "Arabic font", "Naskh styles are easiest to read", "font_ar", Kind::Choice(fonts(crate::assets::ARABIC_FONTS))),
        r(4, "Arabic size", "Bigger is easier to read from a distance", "ar_size", Kind::Slider(80, 140)),
        r(4, "Bangla font", "", "font_bn", Kind::Choice(fonts(crate::assets::BANGLA_FONTS))),
        r(4, "English font", "", "font_en", Kind::Choice(fonts(crate::assets::ENGLISH_FONTS))),
        r(5, "Start with Windows", "", "autostart", Kind::Toggle),
        r(5, "Pause during games and full-screen video", "Uses no CPU while you play or watch", "pause_fullscreen", Kind::Toggle),
        r(5, "Save power on battery", "Stops the camera drift and the system strip", "battery_saver", Kind::Toggle),
        r(
            5,
            "System monitor refresh",
            "Only the small strip is redrawn",
            "monitor_every",
            Kind::Choice(opts(&[("2", "Every 2 seconds"), ("5", "Every 5 seconds"), ("10", "Every 10 seconds")])),
        ),
        r(
            5,
            "Monitors",
            "Each screen can have its own picture",
            "monitors",
            Kind::Choice(opts(&[("same", "Same on all screens"), ("different", "Different on each screen"), ("main", "Main screen only")])),
        ),
        r(5, "About", "", "", Kind::About),
    ]
}

struct Ui {
    hwnd: HWND,
    rows: Vec<Row>,
    /// (row index, sub index, control) for everything in a section.
    ctls: Vec<(usize, usize, HWND)>,
    labels: Vec<(usize, HWND)>,
    hints: Vec<HWND>,
    nav: HWND,
    title: HWND,
    section: usize,
    font: HFONT,
    bold: HFONT,
    small: HFONT,
    big: HFONT,
    white: HBRUSH,
    side: HBRUSH,
    cities: Vec<(String, f64, f64)>,
    scale: f32,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static START: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Opens the window on a given section (0 = Location & times).
pub fn open_at(section: usize) {
    START.with(|s| s.set(section.min(SECTIONS.len() - 1)));
    open();
}

fn value(s: &Settings, key: &str) -> Value {
    serde_json::to_value(s).ok().and_then(|v| v.get(key).cloned()).unwrap_or(Value::Null)
}

fn with(s: &Settings, key: &str, v: Value) -> Settings {
    let mut o = serde_json::to_value(s).unwrap_or(Value::Null);
    o[key] = v;
    serde_json::from_value(o).unwrap_or_else(|_| s.clone())
}

/// A choice's string turned into the field's own JSON type.
fn typed(current: &Value, v: &str) -> Value {
    match current {
        Value::Bool(_) => Value::Bool(v == "true"),
        Value::Number(_) => v.parse::<i64>().map(Value::from).unwrap_or(Value::Null),
        _ => Value::String(v.to_string()),
    }
}

fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn wstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn send(h: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(h, msg, Some(WPARAM(wp)), Some(LPARAM(lp))).0 }
}

fn current() -> Settings {
    crate::with_app(|a| a.s.clone()).unwrap_or_default()
}

/// Opens the window, or brings it forward if it is already open.
pub fn open() {
    let existing = UI.with(|u| u.borrow().as_ref().map(|u| u.hwnd));
    unsafe {
        if let Some(h) = existing {
            let _ = ShowWindow(h, SW_RESTORE);
            let _ = SetForegroundWindow(h);
            return;
        }
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: w!("MiqaatWallpaperSettings"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: crate::tray::icon(32),
            hbrBackground: HBRUSH::default(),
            ..Default::default()
        };
        RegisterClassW(&class);
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("MiqaatWallpaperSettings"),
            w!("Miqaat Wallpaper · Settings"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            100,
            100,
            None,
            None,
            Some(inst.into()),
            None,
        );
    }
}

/// Lets Tab and Enter work in the window (call from the message loop).
pub fn dialog_message(msg: &MSG) -> bool {
    let h = UI.with(|u| u.borrow().as_ref().map(|u| u.hwnd));
    match h {
        Some(h) => unsafe { IsDialogMessageW(h, msg).as_bool() },
        None => false,
    }
}

unsafe fn font(px: i32, weight: i32) -> HFONT {
    CreateFontW(-px, 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"))
}

unsafe fn child(parent: HWND, class: PCWSTR, text: &str, style: u32, id: i32, f: HFONT) -> HWND {
    let h = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class,
        &HSTRING::from(text),
        WINDOW_STYLE(style) | WS_CHILD,
        0,
        0,
        10,
        10,
        Some(parent),
        Some(HMENU(id as isize as _)),
        None,
        None,
    )
    .unwrap_or_default();
    send(h, WM_SETFONT, f.0 as usize, 1);
    h
}

impl Ui {
    fn px(&self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }

    unsafe fn build(&mut self) {
        let s = current();
        let hwnd = self.hwnd;
        let p = |v: f32| (v * self.scale).round() as i32;
        let (cw, ch) = (p(900.0), p(620.0));
        let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
        let _ = AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX, false, WINDOW_EX_STYLE::default(), GetDpiForWindow(hwnd));
        let _ = SetWindowPos(hwnd, None, 0, 0, r.right - r.left, r.bottom - r.top, SWP_NOMOVE | SWP_NOZORDER);

        self.nav = child(hwnd, w!("LISTBOX"), "", (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOTIFY) as u32 | WS_VISIBLE.0 | WS_TABSTOP.0, ID_NAV, self.font);
        let _ = MoveWindow(self.nav, p(12.0), p(56.0), p(204.0), p(520.0), false);
        for name in SECTIONS {
            send(self.nav, LB_ADDSTRING, 0, wstr(name).as_ptr() as isize);
        }
        send(self.nav, LB_SETCURSEL, 0, 0);
        self.title = child(hwnd, w!("STATIC"), SECTIONS[0], WS_VISIBLE.0 | SS_NOPREFIX.0, -1, self.big);
        let _ = MoveWindow(self.title, p(252.0), p(20.0), p(600.0), p(36.0), false);

        let (lx, cx, cwid, row_h) = (252.0, 600.0, 270.0, 46.0);
        let mut y_of = [72.0f32; 6];
        for i in 0..self.rows.len() {
            let row = &self.rows[i];
            let y = y_of[row.sec];
            let tall = matches!(row.kind, Kind::Checks(_) | Kind::Kinds | Kind::About);
            let label = child(hwnd, w!("STATIC"), row.title, SS_NOPREFIX.0, 6000 + i as i32, self.bold);
            let _ = MoveWindow(label, p(lx), p(y), p(330.0), p(20.0), false);
            self.labels.push((i, label));
            let hint_text = if matches!(row.kind, Kind::City) { format!("Now: {} · {:.2}, {:.2}", s.place, s.lat, s.lng) } else { row.hint.to_string() };
            let hint = child(hwnd, w!("STATIC"), &hint_text, SS_NOPREFIX.0 as u32 | SS_ENDELLIPSIS.0 as u32, 5000 + i as i32, self.small);
            let _ = MoveWindow(hint, p(lx), p(y + 20.0), p(if tall { 620.0 } else { 335.0 }), p(18.0), false);
            self.hints.push(hint);
            self.labels.push((i, hint));
            let id = 1000 + i as i32 * 10;
            let cur = value(&s, row.key);
            match &row.kind {
                Kind::Choice(list) => {
                    let c = child(hwnd, w!("COMBOBOX"), "", (CBS_DROPDOWNLIST as u32) | WS_VSCROLL.0 | WS_TABSTOP.0, id, self.font);
                    let _ = MoveWindow(c, p(cx), p(y + 4.0), p(cwid), p(320.0), false);
                    for (k, (v, text)) in list.iter().enumerate() {
                        send(c, CB_ADDSTRING, 0, wstr(text).as_ptr() as isize);
                        if *v == plain(&cur) {
                            send(c, CB_SETCURSEL, k, 0);
                        }
                    }
                    self.ctls.push((i, 0, c));
                }
                Kind::Toggle => {
                    let c = child(hwnd, w!("BUTTON"), "", (BS_AUTOCHECKBOX as u32) | WS_TABSTOP.0, id, self.font);
                    let _ = MoveWindow(c, p(cx + cwid - 22.0), p(y + 8.0), p(22.0), p(22.0), false);
                    send(c, BM_SETCHECK, cur.as_bool().unwrap_or(false) as usize, 0);
                    self.ctls.push((i, 0, c));
                }
                Kind::Slider(lo, hi) => {
                    let c = child(hwnd, TRACKBAR_CLASSW, "", (TBS_HORZ | TBS_NOTICKS) as u32 | WS_TABSTOP.0, id, self.font);
                    let _ = MoveWindow(c, p(cx - 6.0), p(y + 6.0), p(cwid + 12.0), p(28.0), false);
                    send(c, TBM_SETRANGEMIN, 0, *lo as isize);
                    send(c, TBM_SETRANGEMAX, 0, *hi as isize);
                    send(c, TBM_SETPOS, 1, cur.as_i64().unwrap_or(*lo as i64) as isize);
                    self.ctls.push((i, 0, c));
                }
                Kind::Checks(list) => {
                    for (k, (key, text)) in list.iter().enumerate() {
                        let c = child(hwnd, w!("BUTTON"), text, (BS_AUTOCHECKBOX as u32) | WS_TABSTOP.0, id + k as i32, self.font);
                        let _ = MoveWindow(c, p(lx + (k % 3) as f32 * 205.0), p(y + 24.0 + (k / 3) as f32 * 26.0), p(195.0), p(24.0), false);
                        send(c, BM_SETCHECK, value(&s, key).as_bool().unwrap_or(false) as usize, 0);
                        self.ctls.push((i, k, c));
                    }
                }
                Kind::Kinds => {
                    let names = [("quran", "Quran"), ("hadith", "Hadith"), ("dua", "Dua"), ("dhikr", "Dhikr"), ("istighfar", "Istighfar"), ("durood", "Durood")];
                    for (k, (key, text)) in names.iter().enumerate() {
                        let c = child(hwnd, w!("BUTTON"), text, (BS_AUTOCHECKBOX as u32) | WS_TABSTOP.0, id + k as i32, self.font);
                        let _ = MoveWindow(c, p(lx + (k % 3) as f32 * 205.0), p(y + 24.0 + (k / 3) as f32 * 26.0), p(195.0), p(24.0), false);
                        send(c, BM_SETCHECK, s.has(key) as usize, 0);
                        self.ctls.push((i, k, c));
                    }
                }
                Kind::City => {
                    let e = child(hwnd, w!("EDIT"), "", (ES_AUTOHSCROLL as u32) | WS_TABSTOP.0 | WS_BORDER.0, id, self.font);
                    let _ = MoveWindow(e, p(cx), p(y + 4.0), p(cwid - 92.0), p(28.0), false);
                    send(e, 0x1501 /* EM_SETCUEBANNER */, 1, wstr("Search a city…").as_ptr() as isize);
                    let b = child(hwnd, w!("BUTTON"), "Search", (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, ID_SEARCH, self.font);
                    let _ = MoveWindow(b, p(cx + cwid - 84.0), p(y + 3.0), p(84.0), p(30.0), false);
                    self.ctls.push((i, 0, e));
                    self.ctls.push((i, 1, b));
                }
                Kind::Results => {
                    let c = child(hwnd, w!("COMBOBOX"), "", (CBS_DROPDOWNLIST as u32) | WS_VSCROLL.0 | WS_TABSTOP.0, id, self.font);
                    let _ = MoveWindow(c, p(cx), p(y + 4.0), p(cwid), p(320.0), false);
                    self.ctls.push((i, 0, c));
                }
                Kind::PhotoButton => {
                    let b = child(hwnd, w!("BUTTON"), "Choose photo…", (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, id, self.font);
                    let _ = MoveWindow(b, p(cx + cwid - 150.0), p(y + 3.0), p(150.0), p(30.0), false);
                    self.ctls.push((i, 0, b));
                }
                Kind::About => {
                    let text = format!(
                        "Miqaat Wallpaper {} · prayer times from the Miqaat API · Quran: holy-quran library · Hadith: hadith-api · Adhkar: Hisn al-Muslim",
                        crate::install::VERSION
                    );
                    SetWindowTextW(hint, &HSTRING::from(text)).ok();
                    for (k, t) in ["Project page", "Photo credits", "Uninstall…"].iter().enumerate() {
                        let b = child(hwnd, w!("BUTTON"), t, (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, id + k as i32, self.font);
                        let _ = MoveWindow(b, p(lx + k as f32 * 150.0), p(y + 46.0), p(140.0), p(30.0), false);
                        self.ctls.push((i, k, b));
                    }
                }
            }
            y_of[row.sec] += if tall { 92.0 } else { row_h };
        }
        let first = START.with(|s| s.get());
        send(self.nav, LB_SETCURSEL, first, 0);
        self.show(first);
    }

    unsafe fn show(&mut self, sec: usize) {
        self.section = sec;
        SetWindowTextW(self.title, &HSTRING::from(SECTIONS[sec])).ok();
        for (i, h) in self.labels.iter() {
            let _ = ShowWindow(*h, if self.rows[*i].sec == sec { SW_SHOW } else { SW_HIDE });
        }
        for (i, _, h) in self.ctls.iter() {
            let _ = ShowWindow(*h, if self.rows[*i].sec == sec { SW_SHOW } else { SW_HIDE });
        }
        let _ = InvalidateRect(Some(self.hwnd), None, true);
    }

    fn row_of(&self, ctl: HWND) -> Option<(usize, usize)> {
        self.ctls.iter().find(|(_, _, h)| *h == ctl).map(|(i, k, _)| (*i, *k))
    }

    fn ctl(&self, row: usize, sub: usize) -> Option<HWND> {
        self.ctls.iter().find(|(i, k, _)| *i == row && *k == sub).map(|(_, _, h)| *h)
    }
}

/// Reads one control and returns the settings with that change.
unsafe fn read(ui: &Ui, row: usize, sub: usize, h: HWND) -> Option<Settings> {
    let s = current();
    let r = &ui.rows[row];
    Some(match &r.kind {
        Kind::Choice(list) => {
            let i = send(h, CB_GETCURSEL, 0, 0);
            let (v, _) = list.get(usize::try_from(i).ok()?)?;
            if r.key == "scene" && v == "custom" && s.custom_photo.is_empty() {
                return pick_photo(ui.hwnd, &s);
            }
            let mut n = with(&s, r.key, typed(&value(&s, r.key), v));
            if r.key == "scene" {
                n.rotate = "keep".into(); // choosing a picture means "this one"
            }
            n
        }
        Kind::Toggle => with(&s, r.key, Value::Bool(send(h, BM_GETCHECK, 0, 0) == 1)),
        Kind::Slider(..) => with(&s, r.key, Value::from(send(h, TBM_GETPOS, 0, 0) as i64)),
        Kind::Checks(list) => with(&s, list[sub].0, Value::Bool(send(h, BM_GETCHECK, 0, 0) == 1)),
        Kind::Kinds => {
            let names = ["quran", "hadith", "dua", "dhikr", "istighfar", "durood"];
            let mut n = s.clone();
            n.kinds = names.iter().enumerate().filter(|(k, _)| ui.ctl(row, *k).map(|c| send(c, BM_GETCHECK, 0, 0) == 1).unwrap_or(false)).map(|(_, n)| n.to_string()).collect();
            n
        }
        Kind::Results => {
            let i = usize::try_from(send(h, CB_GETCURSEL, 0, 0)).ok()?;
            let (name, lat, lng) = ui.cities.get(i)?.clone();
            let mut n = s.clone();
            n.place = name;
            n.lat = lat;
            n.lng = lng;
            n
        }
        Kind::PhotoButton => return pick_photo(ui.hwnd, &s),
        _ => return None,
    })
}

unsafe fn pick_photo(owner: HWND, s: &Settings) -> Option<Settings> {
    let mut file = [0u16; 1024];
    let filter = wstr("Pictures\0*.jpg;*.jpeg;*.png;*.bmp;*.webp;*.heic\0All files\0*.*\0");
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        ..Default::default()
    };
    if !GetOpenFileNameW(&mut ofn).as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(0);
    let mut n = s.clone();
    n.custom_photo = String::from_utf16_lossy(&file[..len]);
    n.scene = "custom".into();
    n.rotate = "keep".into();
    Some(n)
}

/// Percent-encodes a city name for the geocoding URL.
fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn search(hwnd: HWND, query: String) {
    let target = hwnd.0 as isize;
    std::thread::spawn(move || {
        let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=8&language=en&format=json", encode(&query));
        let found: Vec<(String, f64, f64)> = ureq::get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .call()
            .ok()
            .and_then(|r| r.into_json::<Value>().ok())
            .and_then(|v| v.get("results").and_then(|r| r.as_array()).cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                let name = [c.get("name"), c.get("admin1"), c.get("country")].iter().filter_map(|x| x.and_then(|v| v.as_str())).collect::<Vec<_>>().join(", ");
                Some((name, c.get("latitude")?.as_f64()?, c.get("longitude")?.as_f64()?))
            })
            .collect();
        let ptr = Box::into_raw(Box::new(found)) as isize;
        unsafe {
            if PostMessageW(Some(HWND(target as _)), WM_CITIES, WPARAM(0), LPARAM(ptr)).is_err() {
                drop(Box::from_raw(ptr as *mut Vec<(String, f64, f64)>));
            }
        }
    });
}

fn apply(n: Option<Settings>) {
    if let Some(n) = n {
        crate::apply_settings(n);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
            let px = |v: f32| (v * scale).round() as i32;
            let mut ui = Ui {
                hwnd,
                rows: rows(),
                ctls: vec![],
                labels: vec![],
                hints: vec![],
                nav: HWND::default(),
                title: HWND::default(),
                section: 0,
                font: font(px(15.0), 400),
                bold: font(px(15.0), 600),
                small: font(px(12.5), 400),
                big: font(px(24.0), 600),
                white: CreateSolidBrush(COLORREF(0x00FFFFFF)),
                side: CreateSolidBrush(COLORREF(0x00F3F3F3)),
                cities: vec![],
                scale,
            };
            ui.build();
            UI.with(|u| *u.borrow_mut() = Some(ui));
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            let hdc = HDC(wp.0 as _);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            UI.with(|u| {
                if let Some(ui) = u.borrow().as_ref() {
                    FillRect(hdc, &rc, ui.white);
                    FillRect(hdc, &RECT { right: ui.px(228.0), ..rc }, ui.side);
                }
            });
            LRESULT(1)
        }
        WM_CTLCOLORLISTBOX => {
            // the empty part of the section list matches the side panel
            UI.with(|u| match u.borrow().as_ref() {
                Some(ui) => LRESULT(ui.side.0 as isize),
                None => DefWindowProcW(hwnd, msg, wp, lp),
            })
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let hdc = HDC(wp.0 as _);
            let ctl = HWND(lp.0 as _);
            UI.with(|u| match u.borrow().as_ref() {
                Some(ui) => {
                    SetBkMode(hdc, TRANSPARENT);
                    let hint = ui.hints.contains(&ctl);
                    SetTextColor(hdc, COLORREF(if hint { 0x00707070 } else { 0x001C1C1C }));
                    LRESULT(ui.white.0 as isize)
                }
                None => DefWindowProcW(hwnd, msg, wp, lp),
            })
        }
        WM_MEASUREITEM => {
            let mi = &mut *(lp.0 as *mut MEASUREITEMSTRUCT);
            // Sent while the list is being created, before `UI` is stored: use the DPI directly.
            mi.itemHeight = 40 * GetDpiForWindow(hwnd) / 96;
            LRESULT(1)
        }
        WM_DRAWITEM => {
            let di = &*(lp.0 as *const DRAWITEMSTRUCT);
            UI.with(|u| {
                if let Some(ui) = u.borrow().as_ref() {
                    let hdc = di.hDC;
                    let rc = di.rcItem;
                    FillRect(hdc, &rc, ui.side);
                    let sel = (di.itemState.0 & ODS_SELECTED.0) != 0;
                    if sel {
                        let b = CreateSolidBrush(COLORREF(0x00E8E8E8));
                        let pen = GetStockObject(NULL_PEN);
                        let old_b = SelectObject(hdc, b.into());
                        let old_p = SelectObject(hdc, pen);
                        let r = ui.px(6.0);
                        let _ = RoundRect(hdc, rc.left + ui.px(4.0), rc.top + ui.px(2.0), rc.right - ui.px(4.0), rc.bottom - ui.px(2.0), r, r);
                        SelectObject(hdc, old_b);
                        SelectObject(hdc, old_p);
                        let _ = DeleteObject(b.into());
                        let bar = CreateSolidBrush(COLORREF(0x004B9BC9)); // gold (BGR)
                        let mid = (rc.top + rc.bottom) / 2;
                        FillRect(hdc, &RECT { left: rc.left + ui.px(6.0), top: mid - ui.px(8.0), right: rc.left + ui.px(9.0), bottom: mid + ui.px(8.0) }, bar);
                        let _ = DeleteObject(bar.into());
                    }
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, COLORREF(0x001C1C1C));
                    let old = SelectObject(hdc, if sel { ui.bold } else { ui.font }.into());
                    let mut text: Vec<u16> = SECTIONS.get(di.itemID as usize).unwrap_or(&"").encode_utf16().collect();
                    let mut tr = RECT { left: rc.left + ui.px(20.0), ..rc };
                    DrawTextW(hdc, &mut text, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
                    SelectObject(hdc, old);
                }
            });
            LRESULT(1)
        }
        WM_COMMAND => {
            let id = (wp.0 & 0xFFFF) as i32;
            let code = ((wp.0 >> 16) & 0xFFFF) as u32;
            let ctl = HWND(lp.0 as _);
            if id == ID_NAV && code == LBN_SELCHANGE {
                let sec = send(ctl, LB_GETCURSEL, 0, 0).max(0) as usize;
                UI.with(|u| {
                    if let Some(ui) = u.borrow_mut().as_mut() {
                        ui.show(sec);
                    }
                });
                return LRESULT(0);
            }
            if id == ID_SEARCH {
                let edit = UI.with(|u| u.borrow().as_ref().and_then(|ui| ui.rows.iter().position(|r| matches!(r.kind, Kind::City)).and_then(|i| ui.ctl(i, 0))));
                if let Some(e) = edit {
                    let mut buf = [0u16; 200];
                    let n = GetWindowTextW(e, &mut buf);
                    let q = String::from_utf16_lossy(&buf[..n as usize]).trim().to_string();
                    if !q.is_empty() {
                        search(hwnd, q);
                    }
                }
                return LRESULT(0);
            }
            // Find which row the control belongs to, then let go of the UI before applying.
            let found = UI.with(|u| u.borrow().as_ref().and_then(|ui| ui.row_of(ctl).map(|(i, k)| (i, k, std::mem::discriminant(&ui.rows[i].kind)))));
            let Some((row, sub, _)) = found else { return DefWindowProcW(hwnd, msg, wp, lp) };
            let kind_is = |f: fn(&Kind) -> bool| UI.with(|u| u.borrow().as_ref().map(|ui| f(&ui.rows[row].kind)).unwrap_or(false));
            if kind_is(|k| matches!(k, Kind::About)) {
                match sub {
                    0 => crate::tray::open_url(crate::install::REPO),
                    1 => crate::tray::open_url(&format!("{}/blob/main/CREDITS.md", crate::install::REPO)),
                    _ => {
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(exe).arg("--uninstall").spawn();
                        }
                    }
                }
                return LRESULT(0);
            }
            let changed = (kind_is(|k| matches!(k, Kind::Choice(_) | Kind::Results)) && code == CBN_SELCHANGE)
                || (kind_is(|k| matches!(k, Kind::Toggle | Kind::Checks(_) | Kind::Kinds | Kind::PhotoButton)) && code == BN_CLICKED);
            if changed {
                // Reading may open a file dialog, so it runs without holding the UI borrow.
                let ui_ptr = UI.with(|u| u.borrow().as_ref().map(|ui| ui as *const Ui));
                if let Some(p) = ui_ptr {
                    let n = read(&*p, row, sub, ctl);
                    apply(n);
                    if kind_is(|k| matches!(k, Kind::Results)) {
                        let s = current();
                        UI.with(|u| {
                            if let Some(ui) = u.borrow().as_ref() {
                                if let Some(i) = ui.rows.iter().position(|r| matches!(r.kind, Kind::City)) {
                                    let _ = SetWindowTextW(ui.hints[i], &HSTRING::from(format!("Now: {} · {:.2}, {:.2}", s.place, s.lat, s.lng)));
                                }
                            }
                        });
                    }
                }
            }
            LRESULT(0)
        }
        WM_HSCROLL => {
            // Sliders: wait until the hand rests for a moment before redrawing the wallpaper.
            SetTimer(Some(hwnd), TIMER_SLIDER, 160, None);
            LRESULT(0)
        }
        WM_TIMER if wp.0 == TIMER_SLIDER => {
            let _ = KillTimer(Some(hwnd), TIMER_SLIDER);
            let mut n = current();
            UI.with(|u| {
                if let Some(ui) = u.borrow().as_ref() {
                    for (i, _, h) in &ui.ctls {
                        if let Kind::Slider(..) = ui.rows[*i].kind {
                            n = with(&n, ui.rows[*i].key, Value::from(send(*h, TBM_GETPOS, 0, 0) as i64));
                        }
                    }
                }
            });
            apply(Some(n));
            LRESULT(0)
        }
        WM_CITIES => {
            let found = *Box::from_raw(lp.0 as *mut Vec<(String, f64, f64)>);
            UI.with(|u| {
                if let Some(ui) = u.borrow_mut().as_mut() {
                    let Some(i) = ui.rows.iter().position(|r| matches!(r.kind, Kind::Results)) else { return };
                    let Some(c) = ui.ctl(i, 0) else { return };
                    send(c, CB_RESETCONTENT, 0, 0);
                    if found.is_empty() {
                        send(c, CB_ADDSTRING, 0, wstr("No city found").as_ptr() as isize);
                    }
                    for (name, ..) in &found {
                        send(c, CB_ADDSTRING, 0, wstr(name).as_ptr() as isize);
                    }
                    ui.cities = found;
                    let _ = SetFocus(Some(c));
                    send(c, CB_SHOWDROPDOWN, 1, 0);
                }
            });
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // Simplest correct answer: rebuild at the new scale.
            let _ = DestroyWindow(hwnd);
            open();
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(ui) = UI.with(|u| u.borrow_mut().take()) {
                for f in [ui.font, ui.bold, ui.small, ui.big] {
                    let _ = DeleteObject(f.into());
                }
                let _ = DeleteObject(ui.white.into());
                let _ = DeleteObject(ui.side.into());
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
