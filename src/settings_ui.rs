//! The settings window, drawn with Direct2D in the Windows 11 style.
//!
//! One window: a side bar (find box, six sections, a live status card) and a scrolling page of
//! cards. Everything is drawn in software with the Direct2D the wallpaper already uses, so the
//! window adds no libraries. It follows Windows' light or dark mode. Only the three text boxes
//! (find, city, hex colour) are real Windows edit controls, so typing, the caret and IME work.
//! Rows are described once in `rows()` and read and written through the settings' JSON form.

use crate::settings::Settings;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use windows::core::{w, Interface, Result, BOOL, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::IWICBitmap;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_numerics::{Matrix3x2, Vector2};

type C = D2D1_COLOR_F;
type R = D2D_RECT_F;

const WIN_W: f32 = 900.0;
const WIN_H: f32 = 640.0;
const SIDE_W: f32 = 244.0;
const MX: f32 = 256.0; // page left
const MR: f32 = 872.0; // page right
const SECTIONS: [&str; 6] = ["Location & times", "Reminders", "Picture & light", "Design", "Fonts & language", "System"];
const ROTATE: [(&str, &str); 3] = [("keep", "Keep"), ("daily", "Every day"), ("prayer", "Each prayer")];
const KINDS: [(&str, &str); 6] = [("quran", "Quran"), ("hadith", "Hadith"), ("dua", "Dua"), ("dhikr", "Dhikr"), ("istighfar", "Istighfar"), ("durood", "Durood")];
const SHOWS: [(&str, &str); 5] = [("show_next", "Next prayer"), ("show_reminder", "Reminder"), ("show_special", "Special day"), ("show_times", "Prayer row"), ("show_monitor", "System monitor")];
const PRESETS: [&str; 5] = ["gold", "emerald", "sky", "rose", "pearl"];
const CITY_LIST: usize = usize::MAX;
const WM_CITIES: u32 = WM_APP + 21;
const WM_THUMBS: u32 = WM_APP + 22;
const TIMER_STATUS: usize = 8;
const ID_FIND: i32 = 201;
const ID_CITY: i32 = 202;
const ID_HEX: i32 = 203;

enum Ctl {
    Seg(Vec<(&'static str, &'static str)>),
    Drop(Vec<(String, String)>),
    /// A font list ('a' Arabic, 'b' Bangla, 'e' English) with a sample in the chosen font.
    Font(char),
    Switch,
    Slider(i64, i64),
    Kinds,
    Shows,
    City,
    Pictures,
    Accent,
    /// Which screens show the clock panel (only when several are connected).
    Screens,
    About,
}

struct Row {
    sec: usize,
    group: &'static str,
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    ctl: Ctl,
}

fn rows() -> Vec<Row> {
    let r = |sec, group, title, hint, key, ctl| Row { sec, group, title, hint, key, ctl };
    let seg = |o: &[(&'static str, &'static str)]| Ctl::Seg(o.to_vec());
    let methods = [
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
    ];
    vec![
        r(0, "", "City", "", "place", Ctl::City),
        r(0, "", "Calculation method", "Which authority's angles to use", "method", Ctl::Drop(methods.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect())),
        r(0, "", "Asr time", "Shafi'i, Maliki, Hanbali or Hanafi", "madhab", seg(&[("shafi", "Standard"), ("hanafi", "Hanafi")])),
        r(0, "", "Hijri date", "Moon sighting differs by country; in Bangladesh it is often −1", "hijri_adjust", seg(&[("-2", "−2"), ("-1", "−1"), ("0", "0"), ("1", "+1"), ("2", "+2")])),
        r(0, "Clock and alerts", "Clock", "How times are written", "clock24", seg(&[("false", "12-hour"), ("true", "24-hour")])),
        r(0, "Clock and alerts", "Notify at prayer times", "A small Windows notification, no sound", "notify", Ctl::Switch),
        r(1, "", "Show", "Pick the kinds of reminders", "kinds", Ctl::Kinds),
        r(1, "", "Change every", "A new reminder fades in", "interval", seg(&[("5", "5m"), ("10", "10m"), ("15", "15m"), ("30", "30m"), ("60", "1h")])),
        r(1, "", "Meaning", "Shown under the Arabic", "meaning", seg(&[("en", "English"), ("bn", "বাংলা"), ("none", "Arabic only")])),
        r(1, "Smart timing", "Follow the time of day", "Morning and evening adhkar, adhkar after salah, before sleep", "follow_time", Ctl::Switch),
        r(1, "Smart timing", "Special days", "White Days, Monday/Thursday fasts, Ashura, Arafah, Ramadan…", "special_days", Ctl::Switch),
        r(2, "", "Picture", "One of 20 places, or your own photo", "scene", Ctl::Pictures),
        r(2, "Light", "Light follows the sky", "Rose at Fajr, gold at sunrise, warm at Asr, orange at Maghrib, blue at Isha", "light_follows", Ctl::Switch),
        r(2, "Light", "Night darkness", "How dark the picture gets after Isha", "night_dark", Ctl::Slider(0, 100)),
        r(2, "Light", "Panel shade", "How dark the area behind the text is", "shade", Ctl::Slider(0, 100)),
        r(2, "Effects", "Picture effect", "", "effect", seg(&[("none", "Natural"), ("warm", "Warm"), ("cool", "Cool"), ("mono", "B&W"), ("blur", "Blur")])),
        r(2, "Effects", "Slow camera drift", "A gentle zoom, redrawn once a minute", "drift", Ctl::Switch),
        r(2, "Effects", "Film grain", "", "grain", Ctl::Switch),
        r(3, "", "Layout", "How the information sits on the picture", "layout", seg(&[("column", "Column"), ("centre", "Centre"), ("corner", "Corner"), ("bar", "Bar")])),
        r(3, "", "Side", "Put the text away from your icons", "side", seg(&[("left", "Left"), ("right", "Right")])),
        r(3, "", "Spacing", "Together keeps the clock, reminder and times as one block", "spacing", seg(&[("together", "Together"), ("spread", "Spread")])),
        r(3, "", "Prayer times", "How today's times are shown", "times_style", seg(&[("row", "Row"), ("grid", "Cards"), ("list", "List"), ("timeline", "Timeline")])),
        r(3, "", "System monitor", "CPU, memory, GPU, disk and network", "monitor_style", seg(&[("lines", "Lines"), ("bars", "Bars"), ("rings", "Rings"), ("text", "Compact"), ("off", "Off")])),
        r(3, "", "Behind the text", "", "behind", seg(&[("shade", "Soft shade"), ("glass", "Glass card"), ("none", "Nothing")])),
        r(3, "", "Accent colour", "Headings, the progress line and the current prayer", "accent", Ctl::Accent),
        r(3, "", "Show on wallpaper", "", "", Ctl::Shows),
        r(3, "Clock", "Clock style", "", "clock_style", seg(&[("thin", "Thin"), ("bold", "Bold"), ("serif", "Serif")])),
        r(3, "Clock", "Arabic digits", "Arabic-Indic digits for the clock and times", "arabic_digits", Ctl::Switch),
        r(3, "Clock", "Show seconds", "Redraws only the clock, once a second", "seconds", Ctl::Switch),
        r(3, "Clock", "Text size", "", "text_size", Ctl::Slider(80, 130)),
        r(4, "", "Wallpaper language", "Labels and prayer names", "lang", seg(&[("en", "English"), ("bn", "বাংলা"), ("ar", "العربية")])),
        r(4, "", "Arabic font", "", "font_ar", Ctl::Font('a')),
        r(4, "", "Arabic size", "Bigger is easier to read from a distance", "ar_size", Ctl::Slider(80, 140)),
        r(4, "", "Bangla font", "", "font_bn", Ctl::Font('b')),
        r(4, "", "English font", "", "font_en", Ctl::Font('e')),
        r(5, "", "Start with Windows", "", "autostart", Ctl::Switch),
        r(5, "", "Pause during games and full-screen video", "Uses no CPU while you play or watch", "pause_fullscreen", Ctl::Switch),
        r(5, "", "Save power on battery", "Stops the camera drift and the system strip", "battery_saver", Ctl::Switch),
        r(5, "", "System monitor refresh", "Only the small strip is redrawn", "monitor_every", seg(&[("2", "2 s"), ("5", "5 s"), ("10", "10 s")])),
        r(5, "", "Show the clock and reminders on", "", "panel_on", Ctl::Screens),
        r(5, "", "Monitors", "Each screen can have its own picture", "monitors", seg(&[("same", "Same"), ("different", "Different"), ("main", "Main only")])),
        r(5, "About", "About", "", "", Ctl::About),
    ]
}

fn fonts_of(c: char) -> &'static [crate::assets::Font] {
    match c {
        'a' => crate::assets::ARABIC_FONTS,
        'b' => crate::assets::BANGLA_FONTS,
        _ => crate::assets::ENGLISH_FONTS,
    }
}

fn sample(c: char) -> &'static str {
    match c {
        'a' => "سُبْحَانَ اللَّهِ وَبِحَمْدِهِ",
        'b' => "ফজর বাকি ৭ ঘণ্টা ১০ মিনিট",
        _ => "Fajr in 7h 10m · 4:40 am",
    }
}

// ---------------- settings as JSON ----------------

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

fn current() -> Settings {
    crate::with_app(|a| a.s.clone()).unwrap_or_default()
}

fn apply(n: Settings) {
    crate::apply_settings(n);
}

// ---------------- colour ----------------

#[derive(Clone, Copy)]
struct Theme {
    bg: C,
    card: C,
    line: C,
    ink: C,
    ink2: C,
    hover: C,
    sel: C,
    ctl: C,
    ctlb: C,
    track: C,
    acc: C,
    acc_ink: C,
    shadow: f32,
}

fn hexc(v: u32) -> C {
    C { r: ((v >> 16) & 255) as f32 / 255.0, g: ((v >> 8) & 255) as f32 / 255.0, b: (v & 255) as f32 / 255.0, a: 1.0 }
}

fn alpha(c: C, a: f32) -> C {
    C { a, ..c }
}

fn lum(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

fn colorref(c: C) -> COLORREF {
    COLORREF(((c.b * 255.0) as u32) << 16 | ((c.g * 255.0) as u32) << 8 | (c.r * 255.0) as u32)
}

fn theme(dark: bool, acc: [f32; 3]) -> Theme {
    // In light mode a pale accent would not show on white, so it is deepened a little.
    let acc = if !dark && lum(acc) > 0.6 { acc.map(|v| v * 0.75) } else { acc };
    let acc_ink = if lum(acc) > 0.6 { hexc(0x1b1b1b) } else { hexc(0xffffff) };
    let a = C { r: acc[0], g: acc[1], b: acc[2], a: 1.0 };
    if dark {
        Theme {
            bg: hexc(0x202020),
            card: hexc(0x2b2b2b),
            line: hexc(0x1c1c1c),
            ink: hexc(0xf3f3f3),
            ink2: hexc(0xa8a8a8),
            hover: alpha(hexc(0xffffff), 0.06),
            sel: alpha(hexc(0xffffff), 0.09),
            ctl: hexc(0x373737),
            ctlb: hexc(0x454545),
            track: hexc(0x9a9a9a),
            acc: a,
            acc_ink,
            shadow: 0.3,
        }
    } else {
        Theme {
            bg: hexc(0xf3f3f3),
            card: hexc(0xfbfbfb),
            line: hexc(0xe5e5e5),
            ink: hexc(0x1b1b1b),
            ink2: hexc(0x5d5d5d),
            hover: alpha(hexc(0x000000), 0.04),
            sel: alpha(hexc(0x000000), 0.06),
            ctl: hexc(0xffffff),
            ctlb: hexc(0xd6d6d6),
            track: hexc(0x8a8a8a),
            acc: a,
            acc_ink,
            shadow: 0.08,
        }
    }
}

fn hsv_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h / 60.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)]
}

fn rgb_hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = mx - mn;
    let h = if d == 0.0 {
        0.0
    } else if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    } * 60.0;
    (h, if mx > 0.0 { d / mx } else { 0.0 }, mx)
}

fn to_hex(c: [f32; 3]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    format!("#{:02X}{:02X}{:02X}", b(c[0]), b(c[1]), b(c[2]))
}

fn system_dark() -> bool {
    let (mut v, mut n) = (1u32, 4u32);
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as _),
            Some(&mut n),
        )
        .is_ok()
            && v == 0
    }
}

// ---------------- drawing helpers ----------------

fn rc(l: f32, t: f32, r: f32, b: f32) -> R {
    R { left: l, top: t, right: r, bottom: b }
}

fn inside(r: &R, x: f32, y: f32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

fn v2(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

fn brush(t: &ID2D1RenderTarget, c: C) -> Result<ID2D1SolidColorBrush> {
    unsafe { t.CreateSolidColorBrush(&c, None) }
}

fn rr(t: &ID2D1RenderTarget, r: R, rad: f32, fill: Option<C>, stroke: Option<C>) -> Result<()> {
    unsafe {
        if let Some(c) = fill {
            t.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r, radiusX: rad, radiusY: rad }, &brush(t, c)?);
        }
        if let Some(c) = stroke {
            let h = rc(r.left + 0.5, r.top + 0.5, r.right - 0.5, r.bottom - 0.5);
            t.DrawRoundedRectangle(&D2D1_ROUNDED_RECT { rect: h, radiusX: rad, radiusY: rad }, &brush(t, c)?, 1.0, None);
        }
    }
    Ok(())
}

fn circle(t: &ID2D1RenderTarget, x: f32, y: f32, r: f32, fill: Option<C>, stroke: Option<(C, f32)>) -> Result<()> {
    unsafe {
        let e = D2D1_ELLIPSE { point: v2(x, y), radiusX: r, radiusY: r };
        if let Some(c) = fill {
            t.FillEllipse(&e, &brush(t, c)?);
        }
        if let Some((c, w)) = stroke {
            t.DrawEllipse(&e, &brush(t, c)?, w, None);
        }
    }
    Ok(())
}

fn line(t: &ID2D1RenderTarget, a: (f32, f32), b: (f32, f32), c: C, w: f32) -> Result<()> {
    unsafe { t.DrawLine(v2(a.0, a.1), v2(b.0, b.1), &brush(t, c)?, w, None) };
    Ok(())
}

/// A soft shadow under a floating panel.
fn shadow(t: &ID2D1RenderTarget, r: R, th: &Theme) -> Result<()> {
    for i in 1..=5 {
        let g = i as f32 * 2.5;
        rr(t, rc(r.left - g, r.top - g + 6.0, r.right + g, r.bottom + g + 6.0), 8.0 + g, Some(alpha(hexc(0), th.shadow / i as f32)), None)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Nav(usize),
    Seg(usize, usize),
    Drop(usize),
    Item(usize),
    Switch(usize),
    Slider(usize),
    Chip(usize, usize),
    Thumb(usize),
    Dot(usize),
    Custom,
    Search,
    Button(usize),
    Sv,
    Hue,
    Recent(usize),
    PickOk,
    PickCancel,
    Screen(usize),
    Identify,
    Absorb,
}

struct Hit {
    r: R,
    act: Act,
}

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    None,
    List { row: usize, anchor: R, scroll: f32 },
    Picker,
}

enum Fam<'a> {
    Ui,
    Display,
    Font(&'a (IDWriteFontCollection1, HSTRING)),
}

struct Ui {
    hwnd: HWND,
    scale: f32,
    dark: bool,
    factory: ID2D1Factory,
    dw: IDWriteFactory5,
    loader: IDWriteInMemoryFontFileLoader,
    ui_font: HSTRING,
    disp_font: HSTRING,
    fonts: RefCell<HashMap<String, (IDWriteFontCollection1, HSTRING)>>,
    thumbs: Vec<IWICBitmap>,
    custom_thumb: RefCell<Option<(String, Option<IWICBitmap>)>>,
    rows: Vec<Row>,
    section: usize,
    query: String,
    scroll: f32,
    popup: Popup,
    hover: Option<Act>,
    drag: Option<(Act, R)>,
    slider_val: Option<(usize, i64)>,
    tracking: bool,
    cities: Vec<(String, f64, f64)>,
    searching: bool,
    pick: (f32, f32, f32),
    setting_hex: bool,
    find: HWND,
    city: HWND,
    hex: HWND,
    edit_font: HFONT,
    edit_brush: HBRUSH,
    status: (String, String, String),
    // Filled in while painting:
    hits: RefCell<Vec<Hit>>,
    popup_from: Cell<usize>,
    dy: Cell<f32>,
    content_h: Cell<f32>,
    want_city: Cell<Option<R>>,
    want_hex: Cell<Option<R>>,
    custom_anchor: Cell<R>,
    city_anchor: Cell<R>,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static START: Cell<usize> = const { Cell::new(0) };
}

fn wstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn send(h: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(h, msg, Some(WPARAM(wp)), Some(LPARAM(lp))).0 }
}

fn edit_text(h: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetWindowTextW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn memory_mb() -> f64 {
    unsafe {
        let mut c = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, size).is_ok() {
            return c.PrivateUsage as f64 / 1048576.0;
        }
    }
    0.0
}

fn status_now() -> (String, String, String) {
    let (next, meta) = crate::with_app(|a| {
        let f = a.frame();
        (f.next, f.meta)
    })
    .unwrap_or_default();
    (next, meta, format!("Using {:.0} MB of memory", memory_mb()))
}

impl Ui {
    fn font(&self, c: char, name: &str) -> Option<(IDWriteFontCollection1, HSTRING)> {
        let key = format!("{c}{name}");
        if let Some(f) = self.fonts.borrow().get(&key) {
            return Some(f.clone());
        }
        let list = fonts_of(c);
        let f = list.iter().find(|f| f.name == name).unwrap_or(&list[0]);
        let loaded = crate::render::load_font(&self.dw, &self.loader, f).ok()?;
        self.fonts.borrow_mut().insert(key, loaded.clone());
        Some(loaded)
    }

    #[allow(clippy::too_many_arguments)]
    fn layout(&self, s: &str, fam: &Fam, size: f32, weight: i32, w: f32, h: f32, align: u8) -> Result<IDWriteTextLayout> {
        unsafe {
            let (coll, name): (Option<IDWriteFontCollection>, &HSTRING) = match fam {
                Fam::Ui => (None, &self.ui_font),
                Fam::Display => (None, &self.disp_font),
                Fam::Font(f) => (Some(f.0.cast()?), &f.1),
            };
            let f = self.dw.CreateTextFormat(name, coll.as_ref(), DWRITE_FONT_WEIGHT(weight), DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, w!("en-us"))?;
            f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let sign = self.dw.CreateEllipsisTrimmingSign(&f)?;
            f.SetTrimming(&DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 }, &sign)?;
            let wide: Vec<u16> = s.encode_utf16().collect();
            let l = self.dw.CreateTextLayout(&wide, &f, w.max(1.0), h.max(1.0))?;
            l.SetTextAlignment(match align {
                1 => DWRITE_TEXT_ALIGNMENT_CENTER,
                2 => DWRITE_TEXT_ALIGNMENT_TRAILING,
                _ => DWRITE_TEXT_ALIGNMENT_LEADING,
            })?;
            l.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            Ok(l)
        }
    }

    /// One line of text in `r`, vertically centred, cut with "…" if it doesn't fit.
    #[allow(clippy::too_many_arguments)]
    fn text(&self, t: &ID2D1RenderTarget, s: &str, fam: &Fam, r: R, size: f32, weight: i32, c: C, align: u8) -> Result<()> {
        let l = self.layout(s, fam, size, weight, r.right - r.left, r.bottom - r.top, align)?;
        unsafe { t.DrawTextLayout(v2(r.left, r.top), &l, &brush(t, c)?, D2D1_DRAW_TEXT_OPTIONS_NONE) };
        Ok(())
    }

    fn measure(&self, s: &str, size: f32, weight: i32) -> f32 {
        self.layout(s, &Fam::Ui, size, weight, 10000.0, 100.0, 0)
            .and_then(|l| unsafe {
                let mut m = DWRITE_TEXT_METRICS::default();
                l.GetMetrics(&mut m)?;
                Ok(m.width)
            })
            .unwrap_or(0.0)
    }

    fn hit(&self, r: R, act: Act) {
        let dy = self.dy.get();
        let r = rc(r.left, r.top + dy, r.right, r.bottom + dy);
        if r.bottom < 0.0 || r.top > WIN_H {
            return;
        }
        self.hits.borrow_mut().push(Hit { r, act });
    }

    fn hovered(&self, a: Act) -> bool {
        self.hover == Some(a) || self.drag.map(|d| d.0) == Some(a)
    }

    fn accent_now(&self, s: &Settings) -> [f32; 3] {
        if self.popup == Popup::Picker {
            hsv_rgb(self.pick.0, self.pick.1, self.pick.2)
        } else {
            crate::render::accent(&s.accent)
        }
    }

    // ---------------- paint ----------------

    fn paint(&self, hdc: HDC) -> Result<()> {
        let s = current();
        let th = theme(self.dark, self.accent_now(&s));
        unsafe {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_IGNORE },
                dpiX: 96.0 * self.scale,
                dpiY: 96.0 * self.scale,
                ..Default::default()
            };
            let dc = self.factory.CreateDCRenderTarget(&props)?;
            let mut client = RECT::default();
            GetClientRect(self.hwnd, &mut client)?;
            dc.BindDC(hdc, &client)?;
            dc.BeginDraw();
            let t: ID2D1RenderTarget = dc.cast()?;
            t.Clear(Some(&th.bg));
            self.hits.borrow_mut().clear();
            self.want_city.set(None);
            self.want_hex.set(None);

            self.sidebar(&t, &th)?;

            t.PushAxisAlignedClip(&rc(SIDE_W, 0.0, WIN_W, WIN_H), D2D1_ANTIALIAS_MODE_ALIASED);
            t.SetTransform(&Matrix3x2::translation(0.0, -self.scroll));
            self.dy.set(-self.scroll);
            let bottom = self.page(&t, &th, &s)?;
            t.SetTransform(&Matrix3x2::identity());
            self.dy.set(0.0);
            t.PopAxisAlignedClip();
            self.content_h.set(bottom + 28.0);

            let ch = self.content_h.get();
            if ch > WIN_H {
                let h = (WIN_H * WIN_H / ch).max(40.0);
                let y = self.scroll / (ch - WIN_H) * (WIN_H - h);
                rr(&t, rc(WIN_W - 9.0, y + 4.0, WIN_W - 5.0, y + h - 4.0), 2.0, Some(alpha(th.ink, 0.25)), None)?;
            }

            self.popup_from.set(self.hits.borrow().len());
            match self.popup {
                Popup::List { row, anchor, scroll } => self.list(&t, &th, &s, row, anchor, scroll)?,
                Popup::Picker => self.picker(&t, &th, &s)?,
                Popup::None => {}
            }
            dc.EndDraw(None, None)?;
        }
        Ok(())
    }

    fn icon(&self, t: &ID2D1RenderTarget, i: usize, x: f32, y: f32, c: C) -> Result<()> {
        let w = 1.4;
        let p = |a: f32, b: f32| (x + a, y + b);
        match i {
            0 => {
                circle(t, x + 8.0, y + 6.5, 4.6, None, Some((c, w)))?;
                circle(t, x + 8.0, y + 6.5, 1.4, Some(c), None)?;
                line(t, p(4.4, 9.4), p(8.0, 14.6), c, w)?;
                line(t, p(11.6, 9.4), p(8.0, 14.6), c, w)?;
            }
            1 => {
                rr(t, rc(x + 3.0, y + 2.0, x + 13.0, y + 14.5), 1.5, None, Some(c))?;
                for yy in [6.0, 8.8, 11.6] {
                    line(t, p(5.5, yy), p(10.5, yy), c, w)?;
                }
            }
            2 => {
                rr(t, rc(x + 1.5, y + 3.0, x + 14.5, y + 13.0), 1.5, None, Some(c))?;
                for (a, b) in [((2.5, 11.0), (6.0, 7.5)), ((6.0, 7.5), (9.0, 10.5)), ((9.0, 10.5), (11.0, 8.5)), ((11.0, 8.5), (13.8, 11.2))] {
                    line(t, p(a.0, a.1), p(b.0, b.1), c, w)?;
                }
            }
            3 => {
                for (l, tp, r, b) in [(2.0, 2.0, 7.0, 7.0), (9.0, 2.0, 14.0, 5.0), (9.0, 7.0, 14.0, 14.0), (2.0, 9.0, 7.0, 14.0)] {
                    rr(t, rc(x + l, y + tp, x + r, y + b), 1.0, None, Some(c))?;
                }
            }
            4 => {
                line(t, p(2.5, 13.5), p(6.5, 3.0), c, w)?;
                line(t, p(6.5, 3.0), p(10.5, 13.5), c, w)?;
                line(t, p(4.0, 10.0), p(9.0, 10.0), c, w)?;
                line(t, p(12.0, 7.0), p(15.0, 7.0), c, w)?;
                line(t, p(13.5, 5.5), p(13.5, 8.5), c, w)?;
            }
            _ => {
                rr(t, rc(x + 1.5, y + 2.5, x + 14.5, y + 11.0), 1.2, None, Some(c))?;
                line(t, p(5.5, 14.0), p(10.5, 14.0), c, w)?;
                line(t, p(8.0, 11.0), p(8.0, 14.0), c, w)?;
            }
        }
        Ok(())
    }

    fn sidebar(&self, t: &ID2D1RenderTarget, th: &Theme) -> Result<()> {
        // find box (the edit control sits inside it)
        let f = rc(12.0, 12.0, SIDE_W - 12.0, 46.0);
        rr(t, f, 6.0, Some(th.ctl), Some(th.ctlb))?;
        line(t, (f.left + 6.0, f.bottom - 0.5), (f.right - 6.0, f.bottom - 0.5), if self.query.is_empty() { th.track } else { th.acc }, 1.0)?;
        circle(t, 28.0, 28.0, 4.5, None, Some((th.ink2, 1.4)))?;
        line(t, (31.3, 31.3), (34.5, 34.5), th.ink2, 1.4)?;
        // sections
        for (i, name) in SECTIONS.iter().enumerate() {
            let y = 60.0 + i as f32 * 40.0;
            let r = rc(10.0, y, SIDE_W - 10.0, y + 38.0);
            let on = i == self.section && self.query.is_empty();
            if on {
                rr(t, r, 6.0, Some(th.sel), None)?;
                rr(t, rc(r.left, y + 11.0, r.left + 3.0, y + 27.0), 1.5, Some(th.acc), None)?;
            } else if self.hovered(Act::Nav(i)) {
                rr(t, r, 6.0, Some(th.hover), None)?;
            }
            self.icon(t, i, 24.0, y + 11.0, th.ink)?;
            self.text(t, name, &Fam::Ui, rc(52.0, y, r.right - 8.0, y + 38.0), 14.0, 400, th.ink, 0)?;
            self.hit(r, Act::Nav(i));
        }
        // live status
        let c = rc(12.0, WIN_H - 92.0, SIDE_W - 12.0, WIN_H - 12.0);
        rr(t, c, 8.0, Some(th.card), Some(th.line))?;
        let (a, b, m) = &self.status;
        self.text(t, a, &Fam::Ui, rc(c.left + 12.0, c.top + 10.0, c.right - 10.0, c.top + 30.0), 13.0, 600, th.ink, 0)?;
        self.text(t, b, &Fam::Ui, rc(c.left + 12.0, c.top + 30.0, c.right - 10.0, c.top + 48.0), 12.0, 400, th.ink2, 0)?;
        self.text(t, m, &Fam::Ui, rc(c.left + 12.0, c.top + 48.0, c.right - 10.0, c.top + 66.0), 12.0, 400, th.ink2, 0)?;
        Ok(())
    }

    /// Rows grouped into cards: (group heading, row indexes).
    fn cards(&self) -> Vec<(String, Vec<usize>)> {
        let q = self.query.trim().to_lowercase();
        let mut out: Vec<(String, Vec<usize>)> = vec![];
        for (i, r) in self.rows.iter().enumerate() {
            let show = if q.is_empty() { r.sec == self.section } else { r.title.to_lowercase().contains(&q) || r.hint.to_lowercase().contains(&q) };
            if !show || (matches!(r.ctl, Ctl::Screens) && screens().0.len() < 2) {
                continue;
            }
            let group = if q.is_empty() { r.group.to_string() } else { SECTIONS[r.sec].to_string() };
            match out.last_mut() {
                Some((g, v)) if *g == group => v.push(i),
                _ => out.push((group, vec![i])),
            }
        }
        out
    }

    fn chip_labels(&self, i: usize) -> Vec<&'static str> {
        match self.rows[i].ctl {
            Ctl::Kinds => KINDS.iter().map(|k| k.1).collect(),
            _ => SHOWS.iter().map(|k| k.1).collect(),
        }
    }

    /// Chips flowed into lines 360 wide: lines of (chip index, width).
    fn chip_lines(&self, labels: &[&str]) -> Vec<Vec<(usize, f32)>> {
        let mut lines: Vec<Vec<(usize, f32)>> = vec![vec![]];
        let mut used = 0.0;
        for (k, l) in labels.iter().enumerate() {
            let w = self.measure(l, 12.5, 600) + 26.0;
            if used + w > 360.0 && !lines.last().unwrap().is_empty() {
                lines.push(vec![]);
                used = 0.0;
            }
            lines.last_mut().unwrap().push((k, w));
            used += w + 6.0;
        }
        lines
    }

    fn tile_size(&self) -> (f32, f32) {
        let gw = (MR - 16.0) - (MX + 16.0);
        let w = (gw - 32.0) / 5.0;
        (w, w * 9.0 / 16.0)
    }

    fn row_h(&self, i: usize) -> f32 {
        let r = &self.rows[i];
        match r.ctl {
            Ctl::Pictures => {
                let (_, th) = self.tile_size();
                let n = (crate::assets::SCENES.len() + 1).div_ceil(5) as f32;
                66.0 + n * (th + 8.0) - 8.0 + 16.0
            }
            Ctl::Font(_) => 80.0,
            Ctl::About => 100.0,
            Ctl::Screens => 66.0 + 112.0,
            Ctl::Kinds | Ctl::Shows => (self.chip_lines(&self.chip_labels(i)).len() as f32 * 34.0 + 26.0).max(66.0),
            _ if r.hint.is_empty() && !matches!(r.ctl, Ctl::City) => 56.0,
            _ => 66.0,
        }
    }

    fn page(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings) -> Result<f32> {
        let title = if self.query.trim().is_empty() { SECTIONS[self.section].to_string() } else { "Search results".to_string() };
        self.text(t, &title, &Fam::Display, rc(MX, 14.0, MR, 56.0), 28.0, 600, th.ink, 0)?;
        let mut y = 72.0;
        let cards = self.cards();
        if cards.is_empty() {
            self.text(t, &format!("No settings match “{}”", self.query.trim()), &Fam::Ui, rc(MX, y, MR, y + 24.0), 14.0, 400, th.ink2, 0)?;
            return Ok(y + 24.0);
        }
        for (n, (group, list)) in cards.iter().enumerate() {
            if !group.is_empty() {
                y += if n == 0 { 0.0 } else { 14.0 };
                self.text(t, group, &Fam::Ui, rc(MX, y, MR, y + 22.0), 13.5, 600, th.ink, 0)?;
                y += 28.0;
            } else if n > 0 {
                y += 10.0;
            }
            let hs: Vec<f32> = list.iter().map(|&i| self.row_h(i)).collect();
            let total: f32 = hs.iter().sum();
            rr(t, rc(MX, y, MR, y + total), 8.0, Some(th.card), Some(th.line))?;
            let mut yy = y;
            for (k, &i) in list.iter().enumerate() {
                if k > 0 {
                    line(t, (MX + 1.0, yy), (MR - 1.0, yy), th.line, 1.0)?;
                }
                // draw only rows that can be seen
                let top = yy - self.scroll;
                if top + hs[k] > -10.0 && top < WIN_H + 10.0 {
                    self.row(t, th, s, i, yy, hs[k])?;
                }
                yy += hs[k];
            }
            y += total;
        }
        Ok(y)
    }

    fn row(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, i: usize, y: f32, h: f32) -> Result<()> {
        let r = &self.rows[i];
        let (l, right) = (MX + 16.0, MR - 16.0);
        let band = h.min(if r.hint.is_empty() && !matches!(r.ctl, Ctl::City) { 56.0 } else { 66.0 });
        let cy = y + band / 2.0;
        let cur = value(s, r.key);
        let ctl_left = match &r.ctl {
            Ctl::Seg(o) => {
                let cur = if r.key == "monitor_style" && !s.show_monitor { "off".to_string() } else { plain(&cur) };
                self.seg(t, th, i, o, &cur, right, cy)?
            }
            Ctl::Drop(o) => {
                let label = o.iter().find(|x| x.0 == plain(&cur)).map(|x| x.1.as_str()).unwrap_or("");
                self.dropdown(t, th, i, label, 300.0, right, cy)?
            }
            Ctl::Font(_) => self.dropdown(t, th, i, &plain(&cur), 230.0, right, y + 40.0)?,
            Ctl::Switch => self.switch(t, th, i, cur.as_bool().unwrap_or(false), right, cy)?,
            Ctl::Slider(lo, hi) => {
                let v = match self.slider_val {
                    Some((row, v)) if row == i => v,
                    _ => cur.as_i64().unwrap_or(*lo),
                };
                self.slider(t, th, i, *lo, *hi, v, right, cy)?
            }
            Ctl::Kinds | Ctl::Shows => self.chips(t, th, s, i, right, y, h)?,
            Ctl::City => self.city_ctl(t, th, right, cy)?,
            Ctl::Pictures => {
                self.seg(t, th, i, &ROTATE, &s.rotate, right, cy)?;
                self.pictures(t, th, s, y + 66.0)?;
                right - 330.0
            }
            Ctl::Accent => self.accent_dots(t, th, s, right, cy)?,
            Ctl::Screens => {
                let (rects, main) = screens();
                let n = rects.len();
                self.text(t, r.title, &Fam::Ui, rc(l, cy - 20.0, right, cy), 14.0, 400, th.ink, 0)?;
                self.text(t, &format!("{n} screens found"), &Fam::Ui, rc(l, cy, right, cy + 18.0), 12.0, 400, th.ink2, 0)?;
                let labels: Vec<String> = ["All screens".to_string(), "Main screen".to_string()].into_iter().chain((1..=n).map(|k| format!("Screen {k}"))).collect();
                let values: Vec<String> = ["all".to_string(), "main".to_string()].into_iter().chain((1..=n).map(|k| k.to_string())).collect();
                let opts: Vec<(&str, &str)> = values.iter().zip(labels.iter()).map(|(v, l)| (v.as_str(), l.as_str())).collect();
                self.seg(t, th, i, &opts, &s.panel_on, right, cy)?;
                self.screen_map(t, th, s, &rects, main, y + 66.0)?;
                return Ok(());
            }
            Ctl::About => {
                self.about(t, th, y)?;
                return Ok(());
            }
        };
        let tw = (ctl_left - 20.0).max(l + 120.0);
        let hint = match r.ctl {
            Ctl::City => format!("{} · {:.2}, {:.2}", s.place, s.lat, s.lng),
            _ => r.hint.to_string(),
        };
        if let Ctl::Font(c) = r.ctl {
            self.text(t, r.title, &Fam::Ui, rc(l, y + 10.0, tw, y + 30.0), 14.0, 400, th.ink, 0)?;
            if let Some(f) = self.font(c, &plain(&cur)) {
                let size = if c == 'a' { 19.0 } else { 15.5 };
                self.text(t, sample(c), &Fam::Font(&f), rc(l, y + 32.0, tw, y + 70.0), size, 400, th.ink2, 0)?;
            }
        } else if hint.is_empty() {
            self.text(t, r.title, &Fam::Ui, rc(l, cy - 11.0, tw, cy + 11.0), 14.0, 400, th.ink, 0)?;
        } else {
            self.text(t, r.title, &Fam::Ui, rc(l, cy - 20.0, tw, cy), 14.0, 400, th.ink, 0)?;
            self.text(t, &hint, &Fam::Ui, rc(l, cy, tw, cy + 18.0), 12.0, 400, th.ink2, 0)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn seg(&self, t: &ID2D1RenderTarget, th: &Theme, row: usize, opts: &[(&str, &str)], cur: &str, right: f32, cy: f32) -> Result<f32> {
        let ws: Vec<f32> = opts.iter().map(|o| self.measure(o.1, 13.0, 600) + 24.0).collect();
        let total = ws.iter().sum::<f32>() + 4.0 + 2.0 * (opts.len() as f32 - 1.0);
        let (x0, y0) = (right - total, cy - 16.0);
        rr(t, rc(x0, y0, right, y0 + 32.0), 6.0, Some(th.ctl), Some(th.ctlb))?;
        let mut x = x0 + 2.0;
        for (k, o) in opts.iter().enumerate() {
            let r = rc(x, y0 + 2.0, x + ws[k], y0 + 30.0);
            let on = o.0 == cur;
            if on {
                rr(t, r, 4.0, Some(th.acc), None)?;
            } else if self.hovered(Act::Seg(row, k)) {
                rr(t, r, 4.0, Some(th.hover), None)?;
            }
            self.text(t, o.1, &Fam::Ui, r, 13.0, if on { 600 } else { 400 }, if on { th.acc_ink } else { th.ink }, 1)?;
            self.hit(r, Act::Seg(row, k));
            x += ws[k] + 2.0;
        }
        Ok(x0)
    }

    #[allow(clippy::too_many_arguments)]
    fn dropdown(&self, t: &ID2D1RenderTarget, th: &Theme, row: usize, label: &str, w: f32, right: f32, cy: f32) -> Result<f32> {
        let r = rc(right - w, cy - 17.0, right, cy + 17.0);
        let open = matches!(self.popup, Popup::List { row: x, .. } if x == row);
        rr(t, r, 6.0, Some(th.ctl), Some(if open { th.acc } else { th.ctlb }))?;
        if self.hovered(Act::Drop(row)) {
            rr(t, r, 6.0, Some(th.hover), None)?;
        }
        self.text(t, label, &Fam::Ui, rc(r.left + 12.0, r.top, r.right - 32.0, r.bottom), 13.5, 400, th.ink, 0)?;
        let cx = r.right - 17.0;
        line(t, (cx - 4.0, cy - 2.0), (cx, cy + 2.0), th.ink2, 1.4)?;
        line(t, (cx, cy + 2.0), (cx + 4.0, cy - 2.0), th.ink2, 1.4)?;
        self.hit(r, Act::Drop(row));
        Ok(r.left)
    }

    #[allow(clippy::too_many_arguments)]
    fn switch(&self, t: &ID2D1RenderTarget, th: &Theme, row: usize, on: bool, right: f32, cy: f32) -> Result<f32> {
        let tr = rc(right - 40.0, cy - 10.0, right, cy + 10.0);
        let big = if self.hovered(Act::Switch(row)) { 7.0 } else { 6.0 };
        if on {
            rr(t, tr, 10.0, Some(th.acc), None)?;
            circle(t, right - 10.0, cy, big, Some(th.acc_ink), None)?;
        } else {
            rr(t, tr, 10.0, None, Some(th.track))?;
            circle(t, right - 30.0, cy, big, Some(th.track), None)?;
        }
        self.text(t, if on { "On" } else { "Off" }, &Fam::Ui, rc(right - 100.0, cy - 10.0, right - 50.0, cy + 10.0), 13.0, 400, th.ink2, 2)?;
        self.hit(rc(right - 84.0, cy - 15.0, right, cy + 15.0), Act::Switch(row));
        Ok(right - 90.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn slider(&self, t: &ID2D1RenderTarget, th: &Theme, row: usize, lo: i64, hi: i64, v: i64, right: f32, cy: f32) -> Result<f32> {
        self.text(t, &format!("{v}%"), &Fam::Ui, rc(right - 44.0, cy - 10.0, right, cy + 10.0), 12.5, 400, th.ink2, 2)?;
        let (l, r) = (right - 264.0, right - 56.0);
        rr(t, rc(l, cy - 2.0, r, cy + 2.0), 2.0, Some(alpha(th.track, 0.45)), None)?;
        let f = ((v - lo) as f32 / (hi - lo).max(1) as f32).clamp(0.0, 1.0);
        let px = l + f * (r - l);
        rr(t, rc(l, cy - 2.0, px, cy + 2.0), 2.0, Some(th.acc), None)?;
        circle(t, px, cy, 10.0, Some(th.ctl), Some((th.ctlb, 1.0)))?;
        circle(t, px, cy, if self.hovered(Act::Slider(row)) { 6.5 } else { 5.0 }, Some(th.acc), None)?;
        self.hit(rc(l, cy - 12.0, r, cy + 12.0), Act::Slider(row));
        Ok(l - 12.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn chips(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, row: usize, right: f32, y: f32, h: f32) -> Result<f32> {
        let labels = self.chip_labels(row);
        let lines = self.chip_lines(&labels);
        let mut yy = if lines.len() <= 1 { y + h / 2.0 - 14.0 } else { y + 13.0 };
        let mut left = right;
        for ln in &lines {
            let total: f32 = ln.iter().map(|c| c.1).sum::<f32>() + 6.0 * (ln.len() as f32 - 1.0);
            let mut x = right - total;
            left = left.min(x);
            for &(k, w) in ln {
                let on = match self.rows[row].ctl {
                    Ctl::Kinds => s.has(KINDS[k].0),
                    _ => value(s, SHOWS[k].0).as_bool().unwrap_or(false),
                };
                let r = rc(x, yy, x + w, yy + 28.0);
                if on {
                    rr(t, r, 14.0, Some(th.acc), None)?;
                } else {
                    rr(t, r, 14.0, Some(if self.hovered(Act::Chip(row, k)) { th.sel } else { th.ctl }), Some(th.ctlb))?;
                }
                self.text(t, labels[k], &Fam::Ui, r, 12.5, if on { 600 } else { 400 }, if on { th.acc_ink } else { th.ink }, 1)?;
                self.hit(r, Act::Chip(row, k));
                x += w + 6.0;
            }
            yy += 34.0;
        }
        Ok(left)
    }

    fn city_ctl(&self, t: &ID2D1RenderTarget, th: &Theme, right: f32, cy: f32) -> Result<f32> {
        let label = if self.searching { "Searching…" } else { "Search" };
        let bw = self.measure(label, 13.5, 600) + 30.0;
        let b = rc(right - bw, cy - 17.0, right, cy + 17.0);
        rr(t, b, 6.0, Some(th.acc), None)?;
        if self.hovered(Act::Search) {
            rr(t, b, 6.0, Some(alpha(hexc(0xffffff), 0.14)), None)?;
        }
        self.text(t, label, &Fam::Ui, b, 13.5, 600, th.acc_ink, 1)?;
        self.hit(b, Act::Search);
        let f = rc(b.left - 8.0 - 220.0, cy - 17.0, b.left - 8.0, cy + 17.0);
        rr(t, f, 6.0, Some(th.ctl), Some(th.ctlb))?;
        line(t, (f.left + 6.0, f.bottom - 0.5), (f.right - 6.0, f.bottom - 0.5), th.acc, 1.5)?;
        let dy = self.dy.get();
        self.city_anchor.set(rc(f.left, f.top + dy, f.right, f.bottom + dy));
        let other_list = matches!(self.popup, Popup::List { row, .. } if row != CITY_LIST);
        if f.top + dy > 0.0 && f.bottom + dy < WIN_H && !other_list {
            self.want_city.set(Some(rc(f.left + 10.0, cy - 10.0 + dy, f.right - 8.0, cy + 10.0 + dy)));
        }
        Ok(f.left)
    }

    fn pictures(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, y: f32) -> Result<()> {
        let (tw, thh) = self.tile_size();
        let x0 = MX + 16.0;
        let n = crate::assets::SCENES.len();
        // the user's own photo, decoded once per path
        if !s.custom_photo.is_empty() {
            let stale = self.custom_thumb.borrow().as_ref().map(|(p, _)| p != &s.custom_photo).unwrap_or(true);
            if stale {
                let bmp = crate::render::load_photo(&crate::render::Photo::File(s.custom_photo.clone()), 192, 108, "none", false, false).ok().map(|x| x.0);
                *self.custom_thumb.borrow_mut() = Some((s.custom_photo.clone(), bmp));
            }
        }
        for k in 0..=n {
            let (col, rowi) = ((k % 5) as f32, (k / 5) as f32);
            let x = x0 + col * (tw + 8.0);
            let yy = y + rowi * (thh + 8.0);
            let r = rc(x, yy, x + tw, yy + thh);
            let bmp = if k < n { self.thumbs.get(k).cloned() } else { self.custom_thumb.borrow().as_ref().and_then(|c| c.1.clone()) };
            unsafe {
                match &bmp {
                    Some(b) => {
                        let d = t.CreateBitmapFromWicBitmap(b, None)?;
                        let br = t.CreateBitmapBrush(&d, None, None)?;
                        br.SetTransform(&(Matrix3x2::scale(tw / 192.0, thh / 108.0) * Matrix3x2::translation(x, yy)));
                        t.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r, radiusX: 6.0, radiusY: 6.0 }, &br);
                    }
                    None => rr(t, r, 6.0, Some(th.ctl), Some(th.ctlb))?,
                }
            }
            let name = if k < n { crate::assets::SCENES[k].name.split(" · ").next().unwrap_or("") } else if bmp.is_some() { "Your photo" } else { "+ Your photo" };
            if k < n || bmp.is_some() {
                let lab = rc(x + 7.0, yy + thh - 20.0, x + tw - 4.0, yy + thh - 3.0);
                self.text(t, name, &Fam::Ui, rc(lab.left + 1.0, lab.top + 1.0, lab.right + 1.0, lab.bottom + 1.0), 11.0, 600, alpha(hexc(0), 0.75), 0)?;
                self.text(t, name, &Fam::Ui, lab, 11.0, 600, hexc(0xffffff), 0)?;
            } else {
                self.text(t, name, &Fam::Ui, r, 12.0, 400, th.ink2, 1)?;
            }
            let selected = if k < n { s.scene == crate::assets::SCENES[k].id } else { s.scene == "custom" };
            if selected {
                rr(t, rc(r.left - 2.0, r.top - 2.0, r.right + 2.0, r.bottom + 2.0), 8.0, None, Some(th.acc))?;
                rr(t, rc(r.left - 1.0, r.top - 1.0, r.right + 1.0, r.bottom + 1.0), 7.0, None, Some(th.acc))?;
            } else if self.hovered(Act::Thumb(k)) {
                rr(t, rc(r.left - 1.0, r.top - 1.0, r.right + 1.0, r.bottom + 1.0), 7.0, None, Some(alpha(th.ink, 0.5)))?;
            }
            self.hit(r, Act::Thumb(k));
        }
        Ok(())
    }

    /// The colour wheel on the "custom colour" dot.
    fn wheel(&self, t: &ID2D1RenderTarget, x: f32, y: f32, r: f32) -> Result<()> {
        unsafe {
            for k in 0..12 {
                let (a0, a1) = ((k as f32 * 30.0).to_radians(), ((k + 1) as f32 * 30.0).to_radians());
                let geo = self.factory.CreatePathGeometry()?;
                let sink = geo.Open()?;
                sink.BeginFigure(v2(x, y), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(v2(x + r * a0.cos(), y + r * a0.sin()));
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: v2(x + r * a1.cos(), y + r * a1.sin()),
                    size: D2D_SIZE_F { width: r, height: r },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                let c = hsv_rgb(k as f32 * 30.0, 0.75, 0.95);
                t.FillGeometry(&geo, &brush(t, C { r: c[0], g: c[1], b: c[2], a: 1.0 })?, None);
            }
        }
        Ok(())
    }

    fn accent_dots(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, right: f32, cy: f32) -> Result<f32> {
        let x0 = right - (6.0 * 22.0 + 5.0 * 10.0);
        let custom = crate::render::parse_hex(&s.accent);
        for k in 0..6 {
            let cx = x0 + 11.0 + k as f32 * 32.0;
            let selected = if k < 5 { s.accent == PRESETS[k] && self.popup != Popup::Picker } else { custom.is_some() || self.popup == Popup::Picker };
            let act = if k < 5 { Act::Dot(k) } else { Act::Custom };
            let edge = Some((alpha(hexc(0), 0.15), 1.0));
            if k < 5 {
                let c = crate::render::accent(PRESETS[k]);
                circle(t, cx, cy, 11.0, Some(C { r: c[0], g: c[1], b: c[2], a: 1.0 }), edge)?;
            } else if let (Some(c), false) = (custom, self.popup == Popup::Picker) {
                circle(t, cx, cy, 11.0, Some(C { r: c[0], g: c[1], b: c[2], a: 1.0 }), edge)?;
            } else {
                self.wheel(t, cx, cy, 11.0)?;
                line(t, (cx - 4.5, cy), (cx + 4.5, cy), hexc(0xffffff), 1.8)?;
                line(t, (cx, cy - 4.5), (cx, cy + 4.5), hexc(0xffffff), 1.8)?;
            }
            if selected {
                circle(t, cx, cy, 14.5, None, Some((th.ink, 2.0)))?;
            } else if self.hovered(act) {
                circle(t, cx, cy, 14.0, None, Some((alpha(th.ink, 0.35), 1.5)))?;
            }
            let r = rc(cx - 13.0, cy - 13.0, cx + 13.0, cy + 13.0);
            if k == 5 {
                let dy = self.dy.get();
                self.custom_anchor.set(rc(r.left, r.top + dy, r.right, r.bottom + dy));
            }
            self.hit(r, act);
        }
        Ok(x0 - 6.0)
    }

    /// The connected screens drawn as they are arranged, the ones with the clock outlined.
    #[allow(clippy::too_many_arguments)]
    fn screen_map(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, rects: &[RECT], main: usize, y: f32) -> Result<()> {
        let (l, right) = (MX + 16.0, MR - 16.0);
        let shown = shown_screens(&s.panel_on, rects.len(), main);
        let (x0, y0) = (rects.iter().map(|r| r.left).min().unwrap_or(0), rects.iter().map(|r| r.top).min().unwrap_or(0));
        let (x1, y1) = (rects.iter().map(|r| r.right).max().unwrap_or(1), rects.iter().map(|r| r.bottom).max().unwrap_or(1));
        let (bw, bh) = (right - l - 180.0, 92.0);
        let k = (bw / (x1 - x0).max(1) as f32).min(bh / (y1 - y0).max(1) as f32);
        for (n, r) in rects.iter().enumerate() {
            let gap = 3.0;
            let m = rc(l + (r.left - x0) as f32 * k + gap, y + (r.top - y0) as f32 * k + gap, l + (r.right - x0) as f32 * k - gap, y + (r.bottom - y0) as f32 * k - gap);
            let on = shown.contains(&(n + 1));
            rr(t, m, 6.0, Some(hexc(0x2a3140)), None)?;
            if on {
                // a small panel at the right, like the real clock column
                let w = m.right - m.left;
                rr(t, rc(m.right - w * 0.34, m.top + 8.0, m.right - 8.0, m.bottom - 8.0), 3.0, Some(alpha(hexc(0xffffff), 0.18)), None)?;
            }
            self.text(t, &(n + 1).to_string(), &Fam::Display, m, 22.0, 600, hexc(0xffffff), 1)?;
            let tag = format!("{} × {}{}", r.right - r.left, r.bottom - r.top, if n == main { " · main" } else { "" });
            self.text(t, &tag, &Fam::Ui, rc(m.left + 6.0, m.bottom - 18.0, m.right - 4.0, m.bottom - 2.0), 10.0, 400, alpha(hexc(0xffffff), 0.85), 0)?;
            let ring = if on { th.acc } else if self.hovered(Act::Screen(n + 1)) { alpha(th.ink, 0.5) } else { th.ctlb };
            rr(t, rc(m.left - 1.0, m.top - 1.0, m.right + 1.0, m.bottom + 1.0), 7.0, None, Some(ring))?;
            if on {
                rr(t, rc(m.left - 2.0, m.top - 2.0, m.right + 2.0, m.bottom + 2.0), 8.0, None, Some(ring))?;
            }
            self.hit(m, Act::Screen(n + 1));
        }
        let label = "Identify screens";
        let w = self.measure(label, 13.5, 400) + 30.0;
        let b = rc(right - w, y + 30.0, right, y + 62.0);
        rr(t, b, 6.0, Some(if self.hovered(Act::Identify) { th.sel } else { th.ctl }), Some(th.ctlb))?;
        self.text(t, label, &Fam::Ui, b, 13.5, 400, th.ink, 1)?;
        self.hit(b, Act::Identify);
        Ok(())
    }

    fn about(&self, t: &ID2D1RenderTarget, th: &Theme, y: f32) -> Result<()> {
        let (l, right) = (MX + 16.0, MR - 16.0);
        self.text(t, &format!("Miqaat Wallpaper {}", crate::install::VERSION), &Fam::Ui, rc(l, y + 10.0, right, y + 30.0), 14.0, 400, th.ink, 0)?;
        let src = "Prayer times: Miqaat API · Quran: holy-quran · Hadith: hadith-api · Adhkar: Hisn al-Muslim";
        self.text(t, src, &Fam::Ui, rc(l, y + 30.0, right, y + 48.0), 12.0, 400, th.ink2, 0)?;
        let mut x = l;
        for (k, label) in ["Project page", "Photo credits", "Uninstall…"].iter().enumerate() {
            let w = self.measure(label, 13.5, 400) + 30.0;
            let r = rc(x, y + 58.0, x + w, y + 90.0);
            rr(t, r, 6.0, Some(if self.hovered(Act::Button(k)) { th.sel } else { th.ctl }), Some(th.ctlb))?;
            self.text(t, label, &Fam::Ui, r, 13.5, 400, th.ink, 1)?;
            self.hit(r, Act::Button(k));
            x += w + 8.0;
        }
        Ok(())
    }

    // ---------------- popups ----------------

    /// Items of an open list: (label, selected, font sample).
    fn list_items(&self, row: usize, s: &Settings) -> Vec<(String, bool, Option<char>)> {
        if row == CITY_LIST {
            if self.cities.is_empty() {
                return vec![("No city found".into(), false, None)];
            }
            return self.cities.iter().map(|c| (c.0.clone(), false, None)).collect();
        }
        let cur = plain(&value(s, self.rows[row].key));
        match &self.rows[row].ctl {
            Ctl::Drop(o) => o.iter().map(|(v, l)| (l.clone(), *v == cur, None)).collect(),
            Ctl::Font(c) => fonts_of(*c).iter().map(|f| (f.name.to_string(), f.name == cur, Some(*c))).collect(),
            _ => vec![],
        }
    }

    fn list_geom(&self, row: usize, anchor: R, n: usize) -> (R, f32) {
        let font = row != CITY_LIST && matches!(self.rows[row].ctl, Ctl::Font(_));
        let ih = if font { 40.0 } else { 34.0 };
        let w = if font { 380.0 } else { (anchor.right - anchor.left).max(260.0) };
        let h = n.clamp(1, 9) as f32 * ih + 8.0;
        let x = anchor.left.min(WIN_W - 14.0 - w);
        let y = if anchor.bottom + 4.0 + h <= WIN_H - 8.0 { anchor.bottom + 4.0 } else { (anchor.top - 4.0 - h).max(8.0) };
        (rc(x, y, x + w, y + h), ih)
    }

    #[allow(clippy::too_many_arguments)]
    fn list(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings, row: usize, anchor: R, scroll: f32) -> Result<()> {
        let items = self.list_items(row, s);
        let (r, ih) = self.list_geom(row, anchor, items.len());
        shadow(t, r, th)?;
        rr(t, r, 8.0, Some(th.card), Some(th.ctlb))?;
        self.hit(r, Act::Absorb);
        let enabled = !(row == CITY_LIST && self.cities.is_empty());
        unsafe { t.PushAxisAlignedClip(&rc(r.left, r.top + 4.0, r.right, r.bottom - 4.0), D2D1_ANTIALIAS_MODE_ALIASED) };
        for (k, (label, on, font)) in items.iter().enumerate() {
            let y = r.top + 4.0 + k as f32 * ih - scroll;
            if y + ih < r.top || y > r.bottom {
                continue;
            }
            let ir = rc(r.left + 4.0, y, r.right - 4.0, y + ih);
            if *on {
                rr(t, ir, 4.0, Some(th.sel), None)?;
                rr(t, rc(ir.left, y + ih / 2.0 - 8.0, ir.left + 3.0, y + ih / 2.0 + 8.0), 1.5, Some(th.acc), None)?;
            } else if self.hovered(Act::Item(k)) {
                rr(t, ir, 4.0, Some(th.hover), None)?;
            }
            let end = ir.right - if font.is_some() { 180.0 } else { 10.0 };
            self.text(t, label, &Fam::Ui, rc(ir.left + 12.0, y, end, y + ih), 13.5, 400, if enabled { th.ink } else { th.ink2 }, 0)?;
            if let Some(c) = font {
                if let Some(f) = self.font(*c, label) {
                    let sample = match c {
                        'a' => "بِسْمِ اللَّهِ",
                        'b' => "ফজর ৪:৪০",
                        _ => "Fajr 4:40",
                    };
                    self.text(t, sample, &Fam::Font(&f), rc(ir.right - 175.0, y, ir.right - 12.0, y + ih), if *c == 'a' { 17.0 } else { 14.5 }, 400, th.ink, 2)?;
                }
            }
            if enabled {
                let vis = rc(ir.left, y.max(r.top + 4.0), ir.right, (y + ih).min(r.bottom - 4.0));
                if vis.bottom > vis.top {
                    self.hit(vis, Act::Item(k));
                }
            }
        }
        unsafe { t.PopAxisAlignedClip() };
        Ok(())
    }

    fn picker_rect(&self) -> R {
        let a = self.custom_anchor.get();
        let (w, h) = (264.0, 324.0);
        let x = (a.right + 8.0 - w).clamp(SIDE_W + 8.0, WIN_W - 12.0 - w);
        let y = if a.bottom + 10.0 + h <= WIN_H - 8.0 { a.bottom + 10.0 } else { (a.top - 10.0 - h).max(8.0) };
        rc(x, y, x + w, y + h)
    }

    fn picker(&self, t: &ID2D1RenderTarget, th: &Theme, s: &Settings) -> Result<()> {
        let p = self.picker_rect();
        shadow(t, p, th)?;
        rr(t, p, 10.0, Some(th.card), Some(th.ctlb))?;
        self.hit(p, Act::Absorb);
        let (h, sat, val) = self.pick;
        let cur = hsv_rgb(h, sat, val);
        let curc = C { r: cur[0], g: cur[1], b: cur[2], a: 1.0 };
        let hc = hsv_rgb(h, 1.0, 1.0);
        let huec = C { r: hc[0], g: hc[1], b: hc[2], a: 1.0 };
        unsafe {
            // saturation and brightness square
            let sv = rc(p.left + 12.0, p.top + 12.0, p.right - 12.0, p.top + 162.0);
            rr(t, sv, 6.0, Some(huec), None)?;
            let grad = |a: C, b: C, s0: Vector2, s1: Vector2| -> Result<ID2D1LinearGradientBrush> {
                let stops = [D2D1_GRADIENT_STOP { position: 0.0, color: a }, D2D1_GRADIENT_STOP { position: 1.0, color: b }];
                let coll = t.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
                t.CreateLinearGradientBrush(&D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: s0, endPoint: s1 }, None, &coll)
            };
            let q = D2D1_ROUNDED_RECT { rect: sv, radiusX: 6.0, radiusY: 6.0 };
            t.FillRoundedRectangle(&q, &grad(hexc(0xffffff), alpha(hexc(0xffffff), 0.0), v2(sv.left, 0.0), v2(sv.right, 0.0))?);
            t.FillRoundedRectangle(&q, &grad(alpha(hexc(0), 0.0), hexc(0), v2(0.0, sv.top), v2(0.0, sv.bottom))?);
            let kx = sv.left + sat * (sv.right - sv.left);
            let ky = sv.top + (1.0 - val) * (sv.bottom - sv.top);
            circle(t, kx, ky, 7.0, Some(curc), Some((hexc(0xffffff), 2.0)))?;
            circle(t, kx, ky, 8.5, None, Some((alpha(hexc(0), 0.35), 1.0)))?;
            self.hit(sv, Act::Sv);

            // hue bar
            let hb = rc(p.left + 12.0, p.top + 176.0, p.right - 12.0, p.top + 188.0);
            let stops: Vec<D2D1_GRADIENT_STOP> = (0..=6)
                .map(|k| {
                    let c = hsv_rgb(k as f32 * 60.0, 1.0, 1.0);
                    D2D1_GRADIENT_STOP { position: k as f32 / 6.0, color: C { r: c[0], g: c[1], b: c[2], a: 1.0 } }
                })
                .collect();
            let coll = t.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
            let b = t.CreateLinearGradientBrush(&D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: v2(hb.left, 0.0), endPoint: v2(hb.right, 0.0) }, None, &coll)?;
            t.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: hb, radiusX: 6.0, radiusY: 6.0 }, &b);
            let hx = hb.left + h / 360.0 * (hb.right - hb.left);
            circle(t, hx, (hb.top + hb.bottom) / 2.0, 7.0, Some(huec), Some((hexc(0xffffff), 2.0)))?;
            self.hit(rc(hb.left, hb.top - 6.0, hb.right, hb.bottom + 6.0), Act::Hue);
        }
        // swatch and hex box (the edit control sits inside)
        let y = p.top + 200.0;
        rr(t, rc(p.left + 12.0, y, p.left + 46.0, y + 34.0), 6.0, Some(curc), Some(alpha(hexc(0), 0.15)))?;
        self.text(t, "HEX", &Fam::Ui, rc(p.left + 56.0, y, p.left + 88.0, y + 34.0), 11.0, 600, th.ink2, 0)?;
        let hx = rc(p.left + 90.0, y, p.right - 12.0, y + 34.0);
        rr(t, hx, 6.0, Some(th.ctl), Some(th.ctlb))?;
        line(t, (hx.left + 6.0, hx.bottom - 0.5), (hx.right - 6.0, hx.bottom - 0.5), th.acc, 1.5)?;
        self.want_hex.set(Some(rc(hx.left + 10.0, y + 7.0, hx.right - 8.0, y + 27.0)));
        // recent colours
        let y = p.top + 246.0;
        self.text(t, "Recent", &Fam::Ui, rc(p.left + 12.0, y, p.left + 70.0, y + 20.0), 12.0, 400, th.ink2, 0)?;
        if s.recent_colors.is_empty() {
            self.text(t, "Colours you use appear here", &Fam::Ui, rc(p.left + 70.0, y, p.right - 12.0, y + 20.0), 12.0, 400, th.ink2, 0)?;
        }
        for (k, c) in s.recent_colors.iter().take(6).enumerate() {
            if let Some(rgb) = crate::render::parse_hex(c) {
                let cx = p.left + 80.0 + k as f32 * 26.0;
                circle(t, cx, y + 10.0, 9.0, Some(C { r: rgb[0], g: rgb[1], b: rgb[2], a: 1.0 }), Some((alpha(hexc(0), 0.15), 1.0)))?;
                if self.hovered(Act::Recent(k)) {
                    circle(t, cx, y + 10.0, 12.0, None, Some((alpha(th.ink, 0.4), 1.5)))?;
                }
                self.hit(rc(cx - 11.0, y - 1.0, cx + 11.0, y + 21.0), Act::Recent(k));
            }
        }
        // buttons
        let y = p.bottom - 46.0;
        let ok = rc(p.right - 12.0 - 104.0, y, p.right - 12.0, y + 32.0);
        let cancel = rc(ok.left - 8.0 - 80.0, y, ok.left - 8.0, y + 32.0);
        rr(t, cancel, 6.0, Some(if self.hovered(Act::PickCancel) { th.sel } else { th.ctl }), Some(th.ctlb))?;
        self.text(t, "Cancel", &Fam::Ui, cancel, 13.5, 400, th.ink, 1)?;
        rr(t, ok, 6.0, Some(th.acc), None)?;
        if self.hovered(Act::PickOk) {
            rr(t, ok, 6.0, Some(alpha(hexc(0xffffff), 0.14)), None)?;
        }
        self.text(t, "Use colour", &Fam::Ui, ok, 13.5, 600, th.acc_ink, 1)?;
        self.hit(cancel, Act::PickCancel);
        self.hit(ok, Act::PickOk);
        Ok(())
    }

    fn hit_at(&self, x: f32, y: f32) -> Option<(Act, R)> {
        let hits = self.hits.borrow();
        let from = if self.popup == Popup::None { 0 } else { self.popup_from.get() };
        hits[from.min(hits.len())..].iter().rev().find(|h| inside(&h.r, x, y)).map(|h| (h.act, h.r))
    }
}

// ---------------- window ----------------

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

/// Opens the window on a given section (0 = Location & times).
pub fn open_at(section: usize) {
    START.with(|s| s.set(section.min(SECTIONS.len() - 1)));
    open();
}

/// Enter and Esc in the window (called from the message loop).
pub fn dialog_message(msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let Some((hwnd, city, hex)) = UI.with(|u| u.try_borrow().ok().and_then(|u| u.as_ref().map(|u| (u.hwnd, u.city, u.hex)))) else { return false };
    let ours = msg.hwnd == hwnd || unsafe { GetParent(msg.hwnd).map(|p| p == hwnd).unwrap_or(false) };
    if !ours {
        return false;
    }
    let key = msg.wParam.0 as u16;
    if key == VK_RETURN.0 {
        if msg.hwnd == city {
            start_search();
            return true;
        }
        if msg.hwnd == hex {
            act(Act::PickOk, rc(0.0, 0.0, 0.0, 0.0), 0.0, 0.0);
            return true;
        }
    }
    if key == VK_ESCAPE.0 {
        let had = UI.with(|u| u.borrow_mut().as_mut().map(|u| std::mem::replace(&mut u.popup, Popup::None) != Popup::None)).unwrap_or(false);
        unsafe {
            if had {
                let _ = SetFocus(Some(hwnd));
                let _ = InvalidateRect(Some(hwnd), None, false);
            } else {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        return true;
    }
    false
}

unsafe fn edit(parent: HWND, id: i32, font: HFONT) -> HWND {
    let h = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("EDIT"),
        w!(""),
        WS_CHILD | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
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
    send(h, WM_SETFONT, font.0 as usize, 1);
    h
}

fn set_dark_title(hwnd: HWND, dark: bool) {
    let v = BOOL(dark as i32);
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &v as *const BOOL as _, 4);
    }
}

fn redraw() {
    if let Some(h) = UI.with(|u| u.try_borrow().ok().and_then(|u| u.as_ref().map(|u| u.hwnd))) {
        unsafe {
            let _ = InvalidateRect(Some(h), None, false);
        }
    }
}

/// Moves the edit controls to where the last paint put their boxes.
fn place_edits() {
    let Some((scale, city, hex, wc, wh, popup)) = UI.with(|u| u.borrow().as_ref().map(|u| (u.scale, u.city, u.hex, u.want_city.get(), u.want_hex.get(), u.popup))) else { return };
    let px = |r: R| ((r.left * scale).round() as i32, (r.top * scale).round() as i32, ((r.right - r.left) * scale).round() as i32, ((r.bottom - r.top) * scale).round() as i32);
    unsafe {
        for (h, want) in [(city, wc), (hex, if popup == Popup::Picker { wh } else { None })] {
            match want {
                Some(r) => {
                    let (x, y, w, hh) = px(r);
                    let mut cur = RECT::default();
                    let _ = GetWindowRect(h, &mut cur);
                    let mut pt = POINT { x: cur.left, y: cur.top };
                    let _ = ScreenToClient(GetParent(h).unwrap_or_default(), &mut pt);
                    if !IsWindowVisible(h).as_bool() || pt.x != x || pt.y != y {
                        let _ = SetWindowPos(h, None, x, y, w, hh, SWP_NOZORDER | SWP_SHOWWINDOW | SWP_NOACTIVATE);
                    }
                }
                None => {
                    if IsWindowVisible(h).as_bool() {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                }
            }
        }
    }
}

fn start_search() {
    let Some((hwnd, city)) = UI.with(|u| u.borrow().as_ref().map(|u| (u.hwnd, u.city))) else { return };
    let q = edit_text(city).trim().to_string();
    if q.is_empty() {
        unsafe {
            let _ = SetFocus(Some(city));
        }
        return;
    }
    UI.with(|u| {
        if let Some(u) = u.borrow_mut().as_mut() {
            u.searching = true;
        }
    });
    redraw();
    let target = hwnd.0 as isize;
    std::thread::spawn(move || {
        let enc: String = q.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={enc}&count=8&language=en&format=json");
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

fn pick_photo(owner: HWND, s: &Settings) -> Option<Settings> {
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
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(0);
    let mut n = s.clone();
    n.custom_photo = String::from_utf16_lossy(&file[..len]);
    n.scene = "custom".into();
    n.rotate = "keep".into();
    Some(n)
}

fn set_hex_text(c: [f32; 3]) {
    let Some(hex) = UI.with(|u| {
        u.borrow_mut().as_mut().map(|u| {
            u.setting_hex = true;
            u.hex
        })
    }) else {
        return;
    };
    unsafe {
        let _ = SetWindowTextW(hex, &HSTRING::from(to_hex(c)));
    }
    UI.with(|u| {
        if let Some(u) = u.borrow_mut().as_mut() {
            u.setting_hex = false;
        }
    });
}

/// The mouse moved while dragging a slider or the colour picker.
fn drag_to(a: Act, r: R, x: f32, y: f32) {
    let fx = ((x - r.left) / (r.right - r.left)).clamp(0.0, 1.0);
    let fy = ((y - r.top) / (r.bottom - r.top)).clamp(0.0, 1.0);
    let mut hex = None;
    UI.with(|u| {
        let mut b = u.borrow_mut();
        let Some(u) = b.as_mut() else { return };
        match a {
            Act::Slider(row) => {
                if let Ctl::Slider(lo, hi) = u.rows[row].ctl {
                    u.slider_val = Some((row, lo + ((hi - lo) as f32 * fx).round() as i64));
                }
            }
            Act::Sv => {
                u.pick.1 = fx;
                u.pick.2 = 1.0 - fy;
                hex = Some(hsv_rgb(u.pick.0, u.pick.1, u.pick.2));
            }
            Act::Hue => {
                u.pick.0 = fx * 359.9;
                hex = Some(hsv_rgb(u.pick.0, u.pick.1, u.pick.2));
            }
            _ => {}
        }
    });
    if let Some(c) = hex {
        set_hex_text(c);
    }
    redraw();
}

fn set_popup(p: Popup) {
    UI.with(|u| {
        if let Some(u) = u.borrow_mut().as_mut() {
            u.popup = p;
        }
    })
}

/// What a click does. The UI state is only borrowed briefly, never across a dialog, an
/// edit-control call (they send messages back) or applying settings.
fn act(a: Act, r: R, x: f32, y: f32) {
    let s = current();
    let Some(hwnd) = UI.with(|u| u.borrow().as_ref().map(|u| u.hwnd)) else { return };
    let key_of = |row: usize| UI.with(|u| u.borrow().as_ref().map(|u| u.rows[row].key));
    match a {
        Act::Nav(i) => {
            let find = UI.with(|u| {
                u.borrow_mut().as_mut().map(|u| {
                    u.section = i;
                    u.scroll = 0.0;
                    u.popup = Popup::None;
                    u.query.clear();
                    u.find
                })
            });
            if let Some(f) = find {
                unsafe {
                    let _ = SetWindowTextW(f, w!(""));
                }
            }
        }
        Act::Seg(row, k) => {
            let opt = UI.with(|u| {
                u.borrow().as_ref().and_then(|u| match &u.rows[row].ctl {
                    Ctl::Seg(o) => o.get(k).map(|o| (u.rows[row].key, o.0)),
                    Ctl::Pictures => ROTATE.get(k).map(|o| ("rotate", o.0)),
                    _ => None,
                })
            });
            let screens_row = UI.with(|u| u.borrow().as_ref().map(|u| matches!(u.rows[row].ctl, Ctl::Screens))).unwrap_or(false);
            if screens_row {
                let mut n = s.clone();
                n.panel_on = match k {
                    0 => "all".into(),
                    1 => "main".into(),
                    k => (k - 1).to_string(),
                };
                apply(n);
            } else if let Some(("monitor_style", v)) = opt {
                let mut n = s.clone();
                n.show_monitor = v != "off";
                if v != "off" {
                    n.monitor_style = v.into();
                }
                apply(n);
            } else if let Some((key, v)) = opt {
                apply(with(&s, key, typed(&value(&s, key), v)));
            }
        }
        Act::Drop(row) => {
            let open_now = UI.with(|u| u.borrow().as_ref().map(|u| matches!(u.popup, Popup::List { row: x, .. } if x == row))).unwrap_or(false);
            if open_now {
                set_popup(Popup::None);
            } else {
                // start scrolled so the chosen item shows
                let scroll = UI.with(|u| {
                    u.borrow().as_ref().map(|u| {
                        let items = u.list_items(row, &s);
                        let (_, ih) = u.list_geom(row, r, items.len());
                        let sel = items.iter().position(|i| i.1).unwrap_or(0);
                        let max = items.len().saturating_sub(9) as f32 * ih;
                        (sel as f32 * ih - 3.0 * ih).clamp(0.0, max)
                    })
                });
                set_popup(Popup::List { row, anchor: r, scroll: scroll.unwrap_or(0.0) });
            }
        }
        Act::Item(k) => {
            let row = UI.with(|u| u.borrow().as_ref().and_then(|u| if let Popup::List { row, .. } = u.popup { Some(row) } else { None }));
            set_popup(Popup::None);
            match row {
                Some(CITY_LIST) => {
                    let (c, city) = UI.with(|u| u.borrow().as_ref().map(|u| (u.cities.get(k).cloned(), u.city))).unwrap_or((None, HWND::default()));
                    if let Some((name, lat, lng)) = c {
                        let mut n = s.clone();
                        n.place = name;
                        n.lat = lat;
                        n.lng = lng;
                        apply(n);
                        unsafe {
                            let _ = SetWindowTextW(city, w!(""));
                        }
                    }
                }
                Some(row) => {
                    let v = UI.with(|u| {
                        u.borrow().as_ref().and_then(|u| match &u.rows[row].ctl {
                            Ctl::Drop(o) => o.get(k).map(|o| o.0.clone()),
                            Ctl::Font(c) => fonts_of(*c).get(k).map(|f| f.name.to_string()),
                            _ => None,
                        })
                    });
                    if let (Some(v), Some(key)) = (v, key_of(row)) {
                        apply(with(&s, key, typed(&value(&s, key), &v)));
                    }
                }
                None => {}
            }
        }
        Act::Switch(row) => {
            if let Some(key) = key_of(row) {
                let on = value(&s, key).as_bool().unwrap_or(false);
                apply(with(&s, key, Value::Bool(!on)));
            }
        }
        Act::Slider(_) | Act::Sv | Act::Hue => {
            UI.with(|u| {
                if let Some(u) = u.borrow_mut().as_mut() {
                    u.drag = Some((a, r));
                }
            });
            unsafe { SetCapture(hwnd) };
            drag_to(a, r, x, y);
            return;
        }
        Act::Chip(row, k) => {
            let kinds = UI.with(|u| u.borrow().as_ref().map(|u| matches!(u.rows[row].ctl, Ctl::Kinds))).unwrap_or(false);
            let mut n = s.clone();
            if kinds {
                let key = KINDS[k].0.to_string();
                if n.has(&key) {
                    n.kinds.retain(|x| *x != key);
                } else {
                    n.kinds.push(key);
                }
            } else {
                let key = SHOWS[k].0;
                n = with(&n, key, Value::Bool(!value(&n, key).as_bool().unwrap_or(false)));
            }
            apply(n);
        }
        Act::Thumb(k) => {
            let scenes = crate::assets::SCENES;
            if k < scenes.len() {
                let mut n = s.clone();
                n.scene = scenes[k].id.into();
                n.rotate = "keep".into();
                apply(n);
            } else if !s.custom_photo.is_empty() && s.scene != "custom" && std::path::Path::new(&s.custom_photo).exists() {
                let mut n = s.clone();
                n.scene = "custom".into();
                n.rotate = "keep".into();
                apply(n);
            } else if let Some(n) = pick_photo(hwnd, &s) {
                apply(n);
            }
        }
        Act::Dot(k) => {
            let mut n = s.clone();
            n.accent = PRESETS[k].into();
            apply(n);
        }
        Act::Custom => {
            let open_now = UI.with(|u| u.borrow().as_ref().map(|u| u.popup == Popup::Picker)).unwrap_or(false);
            if open_now {
                set_popup(Popup::None);
            } else {
                let c = crate::render::accent(&s.accent);
                UI.with(|u| {
                    if let Some(u) = u.borrow_mut().as_mut() {
                        u.pick = rgb_hsv(c);
                        u.popup = Popup::Picker;
                    }
                });
                set_hex_text(c);
            }
        }
        Act::Recent(k) => {
            if let Some(c) = s.recent_colors.get(k).and_then(|c| crate::render::parse_hex(c)) {
                UI.with(|u| {
                    if let Some(u) = u.borrow_mut().as_mut() {
                        u.pick = rgb_hsv(c);
                    }
                });
                set_hex_text(c);
            }
        }
        Act::PickOk => {
            let pick = UI.with(|u| {
                u.borrow_mut().as_mut().map(|u| {
                    u.popup = Popup::None;
                    u.pick
                })
            });
            if let Some((h, sat, v)) = pick {
                let hex = to_hex(hsv_rgb(h, sat, v));
                let mut n = s.clone();
                n.recent_colors.retain(|c| !c.eq_ignore_ascii_case(&hex));
                n.recent_colors.insert(0, hex.clone());
                n.recent_colors.truncate(6);
                n.accent = hex;
                apply(n);
            }
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
        }
        Act::PickCancel => set_popup(Popup::None),
        Act::Search => start_search(),
        Act::Button(k) => match k {
            0 => crate::tray::open_url(crate::install::REPO),
            1 => crate::tray::open_url(&format!("{}/blob/main/CREDITS.md", crate::install::REPO)),
            _ => {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--uninstall").spawn();
                }
            }
        },
        Act::Screen(k) => {
            // pick any combination by clicking the screens
            let (rects, main) = screens();
            let mut set = shown_screens(&s.panel_on, rects.len(), main);
            if set.contains(&k) {
                if set.len() > 1 {
                    set.retain(|x| *x != k);
                }
            } else {
                set.push(k);
            }
            set.sort();
            let mut n = s.clone();
            n.panel_on = if set.len() == rects.len() { "all".into() } else { set.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",") };
            apply(n);
        }
        Act::Identify => identify(),
        Act::Absorb => {}
    }
    redraw();
}

fn point(lp: LPARAM, scale: f32) -> (f32, f32) {
    ((lp.0 & 0xFFFF) as i16 as f32 / scale, ((lp.0 >> 16) & 0xFFFF) as i16 as f32 / scale)
}

fn scale_of() -> f32 {
    UI.with(|u| u.borrow().as_ref().map(|u| u.scale)).unwrap_or(1.0)
}

unsafe fn create(hwnd: HWND) -> Result<Ui> {
    let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
    let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
    let dw: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
    let loader = dw.CreateInMemoryFontFileLoader()?;
    dw.RegisterFontFileLoader(&loader)?;
    // Segoe UI Variable on Windows 11, Segoe UI on Windows 10
    let mut sys: Option<IDWriteFontCollection> = None;
    dw.cast::<IDWriteFactory>()?.GetSystemFontCollection(&mut sys, false)?;
    let has = |name: PCWSTR| -> bool {
        let (mut i, mut ok) = (0u32, BOOL(0));
        sys.as_ref().map(|c| c.FindFamilyName(name, &mut i, &mut ok).is_ok() && ok.as_bool()).unwrap_or(false)
    };
    let ui_font = HSTRING::from(if has(w!("Segoe UI Variable Text")) { "Segoe UI Variable Text" } else { "Segoe UI" });
    let disp_font = HSTRING::from(if has(w!("Segoe UI Variable Display")) { "Segoe UI Variable Display" } else { "Segoe UI" });

    let dark = system_dark();
    let edit_font = CreateFontW((-14.0 * scale) as i32, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"));
    let find = edit(hwnd, ID_FIND, edit_font);
    let city = edit(hwnd, ID_CITY, edit_font);
    let hex = edit(hwnd, ID_HEX, edit_font);
    send(find, 0x1501 /* EM_SETCUEBANNER */, 1, wstr("Find a setting").as_ptr() as isize);
    send(city, 0x1501, 1, wstr("Search a city…").as_ptr() as isize);
    send(hex, 0xC5 /* EM_LIMITTEXT */, 7, 0);
    let p = |v: f32| (v * scale).round() as i32;
    let _ = SetWindowPos(find, None, p(42.0), p(19.0), p(SIDE_W - 62.0), p(20.0), SWP_NOZORDER | SWP_SHOWWINDOW);

    let edit_brush = CreateSolidBrush(colorref(theme(dark, [0.0; 3]).ctl));
    let section = START.with(|s| s.replace(0));
    Ok(Ui {
        hwnd,
        scale,
        dark,
        factory,
        dw,
        loader,
        ui_font,
        disp_font,
        fonts: RefCell::new(HashMap::new()),
        thumbs: vec![],
        custom_thumb: RefCell::new(None),
        rows: rows(),
        section,
        query: String::new(),
        scroll: 0.0,
        popup: Popup::None,
        hover: None,
        drag: None,
        slider_val: None,
        tracking: false,
        cities: vec![],
        searching: false,
        pick: (40.0, 0.7, 0.85),
        setting_hex: false,
        find,
        city,
        hex,
        edit_font,
        edit_brush,
        status: status_now(),
        hits: RefCell::new(vec![]),
        popup_from: Cell::new(0),
        dy: Cell::new(0.0),
        content_h: Cell::new(0.0),
        want_city: Cell::new(None),
        want_hex: Cell::new(None),
        custom_anchor: Cell::new(rc(0.0, 0.0, 0.0, 0.0)),
        city_anchor: Cell::new(rc(0.0, 0.0, 0.0, 0.0)),
    })
}

fn set_theme(hwnd: HWND, dark: bool) {
    UI.with(|u| {
        if let Some(u) = u.borrow_mut().as_mut() {
            u.dark = dark;
            unsafe {
                let _ = DeleteObject(u.edit_brush.into());
                u.edit_brush = CreateSolidBrush(colorref(theme(dark, [0.0; 3]).ctl));
            }
        }
    });
    set_dark_title(hwnd, dark);
    unsafe {
        let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let ui = match create(hwnd) {
                Ok(u) => u,
                Err(e) => {
                    crate::log(&format!("settings window: {e}"));
                    return LRESULT(-1);
                }
            };
            let (scale, dark) = (ui.scale, ui.dark);
            UI.with(|u| *u.borrow_mut() = Some(ui));
            // a client area of WIN_W × WIN_H, centred on the work area
            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
            let mut r = RECT { left: 0, top: 0, right: (WIN_W * scale) as i32, bottom: (WIN_H * scale) as i32 };
            let _ = AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE::default(), GetDpiForWindow(hwnd));
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            let _ = GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY), &mut mi);
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            let wa = mi.rcWork;
            let _ = SetWindowPos(hwnd, None, wa.left + (wa.right - wa.left - w) / 2, wa.top + (wa.bottom - wa.top - h) / 2, w, h, SWP_NOZORDER);
            set_dark_title(hwnd, dark);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            SetTimer(Some(hwnd), TIMER_STATUS, 3000, None);
            let _ = PostMessageW(Some(hwnd), WM_THUMBS, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_THUMBS => {
            // Small pictures for the Picture section, decoded once the window is showing.
            let thumbs: Vec<IWICBitmap> = crate::assets::SCENES
                .iter()
                .filter_map(|s| crate::render::load_photo(&crate::render::Photo::Embedded(s.jpg), 192, 108, "none", false, false).ok().map(|x| x.0))
                .collect();
            UI.with(|u| {
                if let Some(u) = u.borrow_mut().as_mut() {
                    u.thumbs = thumbs;
                }
            });
            redraw();
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            UI.with(|u| {
                if let Ok(b) = u.try_borrow() {
                    if let Some(u) = b.as_ref() {
                        if let Err(e) = u.paint(ps.hdc) {
                            crate::log(&format!("settings paint: {e}"));
                        }
                    }
                }
            });
            let _ = EndPaint(hwnd, &ps);
            place_edits();
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wp.0 as _);
            let got = UI.with(|u| u.try_borrow().ok().and_then(|b| b.as_ref().map(|ui| (ui.dark, ui.edit_brush))));
            match got {
                Some((dark, b)) => {
                    let th = theme(dark, [0.0; 3]);
                    SetTextColor(hdc, colorref(th.ink));
                    SetBkColor(hdc, colorref(th.ctl));
                    LRESULT(b.0 as isize)
                }
                None => DefWindowProcW(hwnd, msg, wp, lp),
            }
        }
        WM_LBUTTONDOWN => {
            let _ = SetFocus(Some(hwnd));
            let (x, y) = point(lp, scale_of());
            let found = UI.with(|u| u.borrow().as_ref().map(|u| (u.popup, u.hit_at(x, y))));
            match found {
                // a click outside an open popup only closes it
                Some((p, None)) if p != Popup::None => {
                    set_popup(Popup::None);
                    redraw();
                }
                Some((_, Some((a, r)))) => act(a, r, x, y),
                _ => {}
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lp, scale_of());
            let drag = UI.with(|u| u.borrow().as_ref().and_then(|u| u.drag));
            if let Some((a, r)) = drag {
                drag_to(a, r, x, y);
                return LRESULT(0);
            }
            let changed = UI.with(|u| {
                let mut b = u.borrow_mut();
                let u = b.as_mut()?;
                if !u.tracking {
                    let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                    let _ = TrackMouseEvent(&mut tme);
                    u.tracking = true;
                }
                let h = u.hit_at(x, y).map(|h| h.0).filter(|a| *a != Act::Absorb);
                if h != u.hover {
                    u.hover = h;
                    Some(())
                } else {
                    None
                }
            });
            if changed.is_some() {
                redraw();
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            UI.with(|u| {
                if let Some(u) = u.borrow_mut().as_mut() {
                    u.tracking = false;
                    u.hover = None;
                }
            });
            redraw();
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let done = UI.with(|u| {
                u.borrow_mut().as_mut().and_then(|u| {
                    let d = u.drag.take();
                    let v = u.slider_val.take();
                    d.map(|d| (d.0, v))
                })
            });
            let _ = ReleaseCapture();
            // sliders apply when the hand lets go, so the wallpaper redraws once
            if let Some((Act::Slider(row), Some((_, v)))) = done {
                let key = UI.with(|u| u.borrow().as_ref().map(|u| u.rows[row].key));
                if let Some(key) = key {
                    apply(with(&current(), key, Value::from(v)));
                }
            }
            redraw();
            LRESULT(0)
        }
        WM_SETCURSOR if (lp.0 & 0xFFFF) as u32 == HTCLIENT && wp.0 == hwnd.0 as usize => {
            let hand = UI.with(|u| u.borrow().as_ref().map(|u| u.hover.is_some())).unwrap_or(false);
            SetCursor(LoadCursorW(None, if hand { IDC_HAND } else { IDC_ARROW }).ok());
            LRESULT(1)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
            let mut pt = POINT { x: (lp.0 & 0xFFFF) as i16 as i32, y: ((lp.0 >> 16) & 0xFFFF) as i16 as i32 };
            let _ = ScreenToClient(hwnd, &mut pt);
            let s = current();
            UI.with(|u| {
                if let Some(u) = u.borrow_mut().as_mut() {
                    let x = pt.x as f32 / u.scale;
                    match u.popup {
                        Popup::List { row, anchor, scroll } => {
                            let n = u.list_items(row, &s).len();
                            let (_, ih) = u.list_geom(row, anchor, n);
                            let max = n.saturating_sub(9) as f32 * ih;
                            u.popup = Popup::List { row, anchor, scroll: (scroll - delta * ih * 2.0).clamp(0.0, max) };
                        }
                        Popup::Picker => {}
                        Popup::None if x > SIDE_W => {
                            let max = (u.content_h.get() - WIN_H).max(0.0);
                            u.scroll = (u.scroll - delta * 64.0).clamp(0.0, max);
                        }
                        _ => {}
                    }
                }
            });
            redraw();
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wp.0 & 0xFFFF) as i32;
            let code = ((wp.0 >> 16) & 0xFFFF) as u32;
            if code == EN_CHANGE {
                let text = edit_text(HWND(lp.0 as _));
                if id == ID_FIND {
                    UI.with(|u| {
                        if let Some(u) = u.borrow_mut().as_mut() {
                            u.query = text.trim().to_string();
                            u.scroll = 0.0;
                            u.popup = Popup::None;
                        }
                    });
                    redraw();
                } else if id == ID_HEX {
                    UI.with(|u| {
                        if let Some(u) = u.borrow_mut().as_mut() {
                            if !u.setting_hex {
                                let t = if text.starts_with('#') { text.clone() } else { format!("#{text}") };
                                if let Some(c) = crate::render::parse_hex(&t) {
                                    u.pick = rgb_hsv(c);
                                }
                            }
                        }
                    });
                    redraw();
                }
            }
            LRESULT(0)
        }
        WM_CITIES => {
            let found = *Box::from_raw(lp.0 as *mut Vec<(String, f64, f64)>);
            UI.with(|u| {
                if let Some(u) = u.borrow_mut().as_mut() {
                    u.searching = false;
                    u.cities = found;
                    u.popup = Popup::List { row: CITY_LIST, anchor: u.city_anchor.get(), scroll: 0.0 };
                }
            });
            redraw();
            LRESULT(0)
        }
        WM_TIMER if wp.0 == TIMER_STATUS => {
            let st = status_now();
            let changed = UI.with(|u| u.borrow_mut().as_mut().map(|u| std::mem::replace(&mut u.status, st.clone()) != st)).unwrap_or(false);
            if changed {
                redraw();
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            if lp.0 != 0 && PCWSTR(lp.0 as *const u16).to_string().unwrap_or_default() == "ImmersiveColorSet" {
                set_theme(hwnd, system_dark());
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DPICHANGED => {
            let _ = DestroyWindow(hwnd);
            open();
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(Some(hwnd), TIMER_STATUS);
            if let Some(ui) = UI.with(|u| u.borrow_mut().take()) {
                let _ = DeleteObject(ui.edit_font.into());
                let _ = DeleteObject(ui.edit_brush.into());
                let _ = ui.dw.UnregisterFontFileLoader(&ui.loader);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---------------- screens ----------------

/// The connected screens in screen coordinates, numbered left to right (like `App::screen_order`),
/// and the position of the main one in that list.
fn screens() -> (Vec<RECT>, usize) {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(m, &mut info).as_bool() {
            (*(out.0 as *mut Vec<(RECT, bool)>)).push((info.rcMonitor, info.dwFlags & 1 == 1));
        }
        BOOL(1)
    }
    let mut found: Vec<(RECT, bool)> = vec![];
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    found.sort_by_key(|(r, _)| (r.left, r.top));
    let main = found.iter().position(|(_, p)| *p).unwrap_or(0);
    (found.into_iter().map(|(r, _)| r).collect(), main)
}

/// Screen numbers (1-based) that show the clock for a `panel_on` value.
fn shown_screens(panel_on: &str, n: usize, main: usize) -> Vec<usize> {
    match panel_on {
        "all" => (1..=n).collect(),
        "main" | "" => vec![main + 1],
        list => {
            let v: Vec<usize> = list.split(',').filter_map(|x| x.trim().parse().ok()).filter(|k| (1..=n).contains(k)).collect();
            if v.is_empty() {
                vec![main + 1]
            } else {
                v
            }
        }
    }
}

/// Shows a big number in the middle of every screen for three seconds.
fn identify() {
    unsafe extern "system" fn number(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                let bg = CreateSolidBrush(COLORREF(0x0020_1a16));
                FillRect(hdc, &r, bg);
                let _ = DeleteObject(bg.into());
                let n = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                let font = CreateFontW(-(r.bottom * 3 / 4), 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"));
                let old = SelectObject(hdc, font.into());
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00ff_ffff));
                let mut text: Vec<u16> = n.to_string().encode_utf16().collect();
                DrawTextW(hdc, &mut text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                SelectObject(hdc, old);
                let _ = DeleteObject(font.into());
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
    unsafe {
        let inst = GetModuleHandleW(None).unwrap_or_default();
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(number), hInstance: inst.into(), lpszClassName: w!("MiqaatIdentify"), ..Default::default() });
        for (n, r) in screens().0.iter().enumerate() {
            let size = ((r.bottom - r.top) / 3).max(160);
            let (x, y) = (r.left + (r.right - r.left - size) / 2, r.top + (r.bottom - r.top - size) / 2);
            if let Ok(h) = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
                w!("MiqaatIdentify"),
                w!(""),
                WS_POPUP,
                x,
                y,
                size,
                size,
                None,
                None,
                Some(inst.into()),
                None,
            ) {
                SetWindowLongPtrW(h, GWLP_USERDATA, (n + 1) as isize);
                let _ = SetLayeredWindowAttributes(h, COLORREF(0), 225, LWA_ALPHA);
                let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                SetTimer(Some(h), 1, 3000, None);
            }
        }
    }
}
