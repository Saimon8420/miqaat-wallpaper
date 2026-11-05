//! Drawing the wallpaper with Direct2D and DirectWrite, in software.
//!
//! Everything is drawn on the CPU into the window's device context. A GPU render target would
//! load the graphics driver into the process (about 100 MB on NVIDIA); for a picture that
//! changes once a minute the CPU is plenty and keeps memory small. Each monitor's photo is
//! decoded once, at that monitor's size, and kept as one bitmap.

use std::cell::RefCell;
use std::collections::BTreeMap;
use windows::core::{w, Interface, Result, HSTRING};
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, RECT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows_numerics::{Matrix3x2, Vector2};

use crate::light::Light;
use crate::sysmon::Sample;

/// Where a monitor's picture comes from.
#[derive(Clone, PartialEq)]
pub enum Photo {
    Embedded(&'static [u8]),
    File(String),
    /// A calligraphy scene: its index, whether to mirror it, and the English font for the meaning.
    Calligraphy(usize, bool, String),
}

/// Everything the text panel shows, already in the chosen language.
pub struct Frame {
    pub clock: String,
    pub ampm: String,
    pub seconds: Option<String>,
    pub meta: String,
    pub next: String,
    pub progress: f32,
    pub special: Option<(String, String)>,
    pub kind: String,
    pub arabic: String,
    pub meaning: String,
    pub meaning_bn: bool,
    pub source: String,
    pub times: Vec<(String, String, bool)>,
    pub light: Light,
    pub zoom: f32,
    pub monitor: Option<(Sample, [Vec<f32>; 5], [String; 5])>,
    pub fasting: Option<String>,
    pub event: Option<String>,
    /// Qibla bearing in degrees and its label.
    pub qibla: Option<(f32, String)>,
    /// Fasting, next event and Qibla as values, for the chips, cards and strip styles.
    pub tiles: Vec<crate::extras::Tile>,
    /// Local hours, minutes and seconds, for the dial and the seconds ring.
    pub time: [u32; 3],
}

/// How it looks (from settings).
#[derive(Clone)]
pub struct Style {
    pub layout: String,
    pub side_left: bool,
    pub behind: String,
    pub shade: f32,
    pub accent: [f32; 3],
    pub scale: f32,
    pub ar_scale: f32,
    pub clock_style: String,
    pub rtl_labels: bool,
    pub bn_labels: bool,
    pub show_next: bool,
    pub show_reminder: bool,
    pub show_special: bool,
    pub show_times: bool,
    pub together: bool,
    pub times_style: String,
    pub monitor_style: String,
    pub clock_scale: f32,
    pub progress_style: String,
    pub caps: bool,
    pub tone: [f32; 3],
    /// 0 none, 1 soft, 2 strong.
    pub shadow: u8,
    pub ar_right: bool,
    pub col_w: f32,
    pub glass_light: bool,
    pub corner_r: f32,
    pub dividers: bool,
    pub show_clock: bool,
    /// classic, split, stack, tiles, pill or dial
    pub clock_design: String,
    /// lines, chips, cards or strip
    pub info_style: String,
}

/// A built-in font as a DirectWrite collection, and its family name as DirectWrite sees it
/// (which can differ from the marketing name).
pub fn load_font(dwrite: &IDWriteFactory5, loader: &IDWriteInMemoryFontFileLoader, f: &crate::assets::Font) -> Result<(IDWriteFontCollection1, HSTRING)> {
    unsafe {
        let builder: IDWriteFontSetBuilder1 = dwrite.CreateFontSetBuilder()?.cast()?;
        for data in f.files {
            let file = loader.CreateInMemoryFontFileReference(dwrite, data.as_ptr() as _, data.len() as u32, None::<&windows::core::IUnknown>)?;
            builder.AddFontFile(&file)?;
        }
        let coll = dwrite.CreateFontCollectionFromFontSet(&builder.CreateFontSet()?)?;
        let names = coll.GetFontFamily(0)?.GetFamilyNames()?;
        let mut buf = vec![0u16; names.GetStringLength(0)? as usize + 1];
        names.GetString(0, &mut buf)?;
        buf.pop();
        Ok((coll, HSTRING::from_wide(&buf)))
    }
}

/// "#RRGGBB" -> 0..1 RGB.
pub fn parse_hex(s: &str) -> Option<[f32; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16 & 255) as f32 / 255.0, (n >> 8 & 255) as f32 / 255.0, (n & 255) as f32 / 255.0])
}

/// A preset name or a custom "#RRGGBB" colour.
pub fn accent(name: &str) -> [f32; 3] {
    if let Some(c) = parse_hex(name) {
        return c;
    }
    match name {
        "emerald" => [0.40, 0.82, 0.64],
        "sky" => [0.52, 0.76, 0.96],
        "rose" => [0.94, 0.62, 0.70],
        "pearl" => [0.95, 0.93, 0.88],
        _ => [0.87, 0.73, 0.45], // gold
    }
}

struct Fonts {
    ar: (IDWriteFontCollection1, HSTRING),
    en: (IDWriteFontCollection1, HSTRING),
    bn: (IDWriteFontCollection1, HSTRING),
    clock: (IDWriteFontCollection1, HSTRING),
}

pub struct Renderer {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory5,
    loader: IDWriteInMemoryFontFileLoader,
    fonts: Fonts,
    /// One decoded photo per distinct (source, size); `of_monitor[i]` picks one per monitor.
    photos: Vec<IWICBitmap>,
    blurred: Vec<IWICBitmap>,
    of_monitor: Vec<usize>,
    /// Rectangles found during the last paint (client coordinates), for partial repaints.
    pub clock_rect: RefCell<BTreeMap<usize, RECT>>,
    pub strip_rect: RefCell<BTreeMap<usize, RECT>>,
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn rect(l: f32, t: f32, r: f32, b: f32) -> D2D_RECT_F {
    D2D_RECT_F { left: l, top: t, right: r, bottom: b }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// A piece of text placed on the wallpaper.
struct Piece {
    layout: IDWriteTextLayout,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: D2D1_COLOR_F,
}

/// Bounding box of some pieces, grown by `pad`.
fn bounds(ps: &[&Piece], pad: f32) -> Option<D2D_RECT_F> {
    let mut r: Option<D2D_RECT_F> = None;
    for p in ps {
        let q = rect(p.x, p.y, p.x + p.w, p.y + p.h);
        r = Some(match r {
            None => q,
            Some(r) => rect(r.left.min(q.left), r.top.min(q.top), r.right.max(q.right), r.bottom.max(q.bottom)),
        });
    }
    r.map(|r| rect(r.left - pad, r.top - pad, r.right + pad, r.bottom + pad))
}


/// A drawing made of simple parts, laid out relative to its own top-left corner: the clock
/// designs (stacked, tiles, pill, dial) and the info styles (chips, cards, strip).
struct Art {
    w: f32,
    h: f32,
    items: Vec<ArtItem>,
}

enum ArtItem {
    Text(Piece),
    Rect { r: D2D_RECT_F, radius: f32, fill: D2D1_COLOR_F, stroke: Option<D2D1_COLOR_F> },
    Line { a: Vector2, b: Vector2, w: f32, c: D2D1_COLOR_F },
    Circle { c: Vector2, r: f32, fill: Option<D2D1_COLOR_F>, stroke: Option<(D2D1_COLOR_F, f32)> },
    /// An arc from angle `a0` to `a1` (radians, 0 = right, clockwise).
    Arc { c: Vector2, r: f32, a0: f32, a1: f32, w: f32, col: D2D1_COLOR_F },
}

fn v2(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

impl Renderer {
    /// `photos`: one source per monitor with that monitor's size.
    #[allow(clippy::too_many_arguments)]
    pub fn new(photos: &[(Photo, u32, u32)], effect: &str, grain: bool, glass: bool, glass_blur: u32, font_ar: &str, font_bn: &str, font_en: &str, font_clock: &str) -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
            let loader = dwrite.CreateInMemoryFontFileLoader()?;
            dwrite.RegisterFontFileLoader(&loader)?;
            let pick = |list: &'static [crate::assets::Font], name: &str| list.iter().find(|f| f.name == name).unwrap_or(&list[0]);
            let load = |f: &crate::assets::Font| load_font(&dwrite, &loader, f);
            let fonts = Fonts {
                ar: load(pick(crate::assets::ARABIC_FONTS, font_ar))?,
                bn: load(pick(crate::assets::BANGLA_FONTS, font_bn))?,
                en: load(pick(crate::assets::ENGLISH_FONTS, font_en))?,
                clock: load(pick(crate::assets::ENGLISH_FONTS, font_clock))?,
            };

            let mut keys: Vec<(Photo, u32, u32)> = vec![];
            let mut of_monitor = vec![];
            for p in photos {
                let i = match keys.iter().position(|k| k == p) {
                    Some(i) => i,
                    None => {
                        keys.push(p.clone());
                        keys.len() - 1
                    }
                };
                of_monitor.push(i);
            }
            let mut decoded = vec![];
            let mut blurred = vec![];
            for (src, w, h) in &keys {
                let (photo, soft) = load_photo(src, *w, *h, effect, grain, glass, glass_blur)?;
                decoded.push(photo);
                if let Some(s) = soft {
                    blurred.push(s);
                }
            }
            Ok(Renderer {
                factory,
                dwrite,
                loader,
                fonts,
                photos: decoded,
                blurred,
                of_monitor,
                clock_rect: RefCell::new(BTreeMap::new()),
                strip_rect: RefCell::new(BTreeMap::new()),
            })
        }
    }

    fn format(&self, which: char, size: f32, weight: DWRITE_FONT_WEIGHT, rtl: bool) -> Result<IDWriteTextFormat> {
        let (coll, name) = match which {
            'a' => &self.fonts.ar,
            'b' => &self.fonts.bn,
            's' | 'c' => &self.fonts.clock,
            _ => &self.fonts.en,
        };
        unsafe {
            let f = self.dwrite.CreateTextFormat(name, &coll.cast::<IDWriteFontCollection>()?, weight, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size.max(1.0), w!("en-us"))?;
            if rtl {
                f.SetReadingDirection(DWRITE_READING_DIRECTION_RIGHT_TO_LEFT)?;
            }
            f.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            Ok(f)
        }
    }

    /// Lays out `text` in a box `width` wide. `align`: 0 left, 1 centre, 2 right (on screen).
    fn piece(&self, text: &str, f: &IDWriteTextFormat, width: f32, align: u8, spacing: f32, color: D2D1_COLOR_F) -> Result<Piece> {
        unsafe {
            let rtl = f.GetReadingDirection() == DWRITE_READING_DIRECTION_RIGHT_TO_LEFT;
            let l = self.dwrite.CreateTextLayout(&wide(text), f, width.max(1.0), 10000.0)?;
            let a = match (align, rtl) {
                (1, _) => DWRITE_TEXT_ALIGNMENT_CENTER,
                (0, false) | (2, true) => DWRITE_TEXT_ALIGNMENT_LEADING,
                _ => DWRITE_TEXT_ALIGNMENT_TRAILING,
            };
            l.SetTextAlignment(a)?;
            if spacing != 0.0 {
                let l1: IDWriteTextLayout1 = l.cast()?;
                l1.SetCharacterSpacing(0.0, spacing, 0.0, DWRITE_TEXT_RANGE { startPosition: 0, length: text.encode_utf16().count() as u32 })?;
            }
            let mut m = DWRITE_TEXT_METRICS::default();
            l.GetMetrics(&mut m)?;
            Ok(Piece { layout: l, x: 0.0, y: 0.0, w: width, h: m.height, color })
        }
    }

    /// The clock on one line that never wraps (the font shrinks to fit, down to half). The size is chosen from the widest possible time (every
    /// digit an 8 or a 0, whichever is wider) and digits use equal-width figures, so the clock,
    /// am/pm and the lines around it stay still while the seconds tick.
    #[allow(clippy::too_many_arguments)]
    fn clock_line(&self, text: &str, which: char, size: f32, weight: DWRITE_FONT_WEIGHT, width: f32, align: u8, color: D2D1_COLOR_F, bold: usize) -> Result<(Piece, f32)> {
        let tnum = |p: &Piece, n: usize| -> Result<()> {
            unsafe {
                let ty = self.dwrite.CreateTypography()?;
                ty.AddFontFeature(DWRITE_FONT_FEATURE { nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES, parameter: 1 })?;
                p.layout.SetTypography(&ty, DWRITE_TEXT_RANGE { startPosition: 0, length: n as u32 })?;
                // "Bold hours": the first `bold` characters heavier than the rest
                if bold > 0 {
                    p.layout.SetFontWeight(DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_RANGE { startPosition: 0, length: bold as u32 })?;
                }
            }
            Ok(())
        };
        let swap = |d: char| text.chars().map(|c| if c.is_numeric() { d } else { c }).collect::<String>();
        let n = text.encode_utf16().count();
        let mut s = size;
        loop {
            let f = self.format(which, s, weight, false)?;
            unsafe { f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)? };
            let mut widest = 0.0f32;
            for d in ['0', '8'] {
                let p = self.piece(&swap(d), &f, width, align, 0.0, color)?;
                tnum(&p, n)?;
                widest = widest.max(self.width(&p)?);
            }
            if widest <= width || s <= size * 0.5 {
                let p = self.piece(text, &f, width, align, 0.0, color)?;
                tnum(&p, n)?;
                return Ok((p, widest));
            }
            s *= 0.95;
        }
    }

    /// An empty piece that only keeps a place in the layout.
    fn blank(&self, h: f32, w: f32) -> Result<Piece> {
        let mut p = self.piece("", &self.format('e', 12.0, DWRITE_FONT_WEIGHT_NORMAL, false)?, w, 0, 0.0, rgba(0.0, 0.0, 0.0, 0.0))?;
        p.h = h;
        Ok(p)
    }

    /// A single line of text that does not wrap, with equal-width digits.
    #[allow(clippy::too_many_arguments)]
    fn label_piece(&self, text: &str, which: char, size: f32, weight: DWRITE_FONT_WEIGHT, rtl: bool, align: u8, width: f32, color: D2D1_COLOR_F) -> Result<Piece> {
        let fmt = self.format(which, size, weight, rtl)?;
        unsafe { fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)? };
        let p = self.piece(text, &fmt, width.max(1.0), align, 0.0, color)?;
        unsafe {
            let ty = self.dwrite.CreateTypography()?;
            ty.AddFontFeature(DWRITE_FONT_FEATURE { nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES, parameter: 1 })?;
            p.layout.SetTypography(&ty, DWRITE_TEXT_RANGE { startPosition: 0, length: text.encode_utf16().count() as u32 })?;
        }
        Ok(p)
    }

    /// The clock drawn as stacked numbers, flip tiles, a pill or a dial. Every number that
    /// changes sits in a box of fixed size, so nothing moves while the seconds tick.
    #[allow(clippy::too_many_arguments)]
    fn clock_art(&self, design: &str, hours: &str, minutes: &str, secs: Option<&str>, ampm: &str, time: [u32; 3], size: f32, weight: DWRITE_FONT_WEIGHT, width: f32, tone: [f32; 3], acc: D2D1_COLOR_F, u: f32) -> Result<Art> {
        let [tr, tg, tb] = tone;
        let ink = |a: f32| rgba(tr, tg, tb, a);
        let mut items = vec![];
        let at = |mut p: Piece, x: f32, y: f32| {
            p.x = x;
            p.y = y;
            ArtItem::Text(p)
        };
        let (w, h) = match design {
            "stack" => {
                // a big hour, with minutes, seconds and am/pm stacked beside it
                let hp = self.label_piece(hours, 'c', size * 1.25, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, 0, width, ink(1.0))?;
                let hw = self.width(&hp)?.max(size * 0.62);
                let col = hw + 0.9 * u;
                let mp = self.label_piece(minutes, 'c', size * 0.6, weight, false, 0, width, ink(1.0))?;
                let mut y = hp.h * 0.12;
                let mh = mp.h;
                items.push(at(mp, col, y));
                y += mh * 0.92;
                if let Some(s) = secs {
                    let sp = self.label_piece(s, 'c', size * 0.27, DWRITE_FONT_WEIGHT_LIGHT, false, 0, width, ink(0.6))?;
                    let sh = sp.h;
                    items.push(at(sp, col + 0.1 * u, y));
                    y += sh;
                }
                if !ampm.is_empty() {
                    let ap = self.label_piece(&ampm.to_uppercase(), 'e', size * 0.15, DWRITE_FONT_WEIGHT_NORMAL, false, 0, width, ink(0.7))?;
                    items.push(at(ap, col + 0.12 * u, y + 0.1 * u));
                }
                let h = hp.h * 0.95;
                items.insert(0, at(hp, 0.0, 0.0));
                (col + size * 0.75, h)
            }
            "tiles" => {
                // flip-clock tiles: hh : mm (: ss), one digit per tile
                let hh: String = if hours.chars().count() < 2 { format!("{}{}", if hours.chars().all(|c| c.is_ascii_digit()) { '0' } else { '٠' }, hours) } else { hours.to_string() };
                let (tw, th) = (size * 0.52, size * 0.84);
                let gap = size * 0.07;
                let mut x = 0.0;
                let tile = |items: &mut Vec<ArtItem>, d: char, x: f32, scale: f32| -> Result<f32> {
                    let (w, hgt) = (tw * scale, th * scale);
                    let y = (th - hgt) / 2.0;
                    items.push(ArtItem::Rect { r: rect(x, y, x + w, y + hgt), radius: 0.16 * w, fill: ink(0.09), stroke: Some(ink(0.14)) });
                    let p = self.label_piece(&d.to_string(), 'c', size * 0.6 * scale, weight, false, 1, w, ink(if scale < 1.0 { 0.75 } else { 1.0 }))?;
                    let ph = p.h;
                    items.push(at(p, x, y + (hgt - ph) / 2.0));
                    items.push(ArtItem::Line { a: v2(x, y + hgt / 2.0), b: v2(x + w, y + hgt / 2.0), w: (0.06 * u).max(1.0), c: rgba(0.0, 0.0, 0.0, 0.3) });
                    Ok(x + w + gap)
                };
                let colon = |items: &mut Vec<ArtItem>, x: f32| -> f32 {
                    for k in [0.38, 0.62] {
                        items.push(ArtItem::Circle { c: v2(x + size * 0.09, th * k), r: size * 0.035, fill: Some(ink(0.6)), stroke: None });
                    }
                    x + size * 0.18 + gap
                };
                for d in hh.chars() {
                    x = tile(&mut items, d, x, 1.0)?;
                }
                x = colon(&mut items, x);
                for d in minutes.chars() {
                    x = tile(&mut items, d, x, 1.0)?;
                }
                if let Some(s) = secs {
                    x = colon(&mut items, x);
                    for d in s.chars() {
                        x = tile(&mut items, d, x, 0.66)?;
                    }
                }
                if !ampm.is_empty() {
                    let ap = self.label_piece(ampm, 'e', size * 0.17, DWRITE_FONT_WEIGHT_LIGHT, false, 0, width, ink(0.75))?;
                    let ah = ap.h;
                    let aw = self.width(&ap)?;
                    items.push(at(ap, x + 0.2 * u, th * 0.7 - ah));
                    x += 0.2 * u + aw;
                }
                (x, th)
            }
            "pill" => {
                // the time in a rounded capsule; a small ring fills with the seconds
                let fs = size * 0.62;
                let (tp, tw) = self.clock_line(&format!("{hours}:{minutes}"), 'c', fs, weight, width, 0, ink(1.0), 0)?;
                let pad = fs * 0.5;
                let hgt = tp.h * 1.08;
                let mut x = pad;
                let th = tp.h;
                items.push(at(tp, x, (hgt - th) / 2.0));
                x += tw + fs * 0.25;
                if secs.is_some() {
                    let r = fs * 0.22;
                    let c = v2(x + r, hgt / 2.0);
                    items.push(ArtItem::Circle { c, r, fill: None, stroke: Some((ink(0.2), fs * 0.07)) });
                    let frac = time[2] as f32 / 60.0;
                    if frac > 0.0 {
                        items.push(ArtItem::Arc { c, r, a0: -std::f32::consts::FRAC_PI_2, a1: -std::f32::consts::FRAC_PI_2 + frac * std::f32::consts::TAU, w: fs * 0.07, col: acc });
                    }
                    x += 2.0 * r + fs * 0.25;
                }
                if !ampm.is_empty() {
                    let ap = self.label_piece(ampm, 'e', fs * 0.28, DWRITE_FONT_WEIGHT_LIGHT, false, 0, width, ink(0.75))?;
                    let (ah, aw) = (ap.h, self.width(&ap)?);
                    items.push(at(ap, x, hgt * 0.62 - ah / 2.0));
                    x += aw;
                }
                let w = x + pad;
                items.insert(0, ArtItem::Rect { r: rect(0.0, 0.0, w, hgt), radius: hgt / 2.0, fill: ink(0.08), stroke: Some(ink(0.15)) });
                (w, hgt)
            }
            _ => {
                // an analog dial
                let r = size * 0.78;
                let c = v2(r, r);
                items.push(ArtItem::Circle { c, r, fill: Some(ink(0.05)), stroke: Some((ink(0.18), (0.08 * u).max(1.0))) });
                for i in 0..60 {
                    let a = i as f32 * 6.0_f32.to_radians();
                    let big = i % 5 == 0;
                    let (r0, r1) = (r * if big { 0.83 } else { 0.9 }, r * 0.95);
                    items.push(ArtItem::Line { a: v2(c.X + a.sin() * r0, c.Y - a.cos() * r0), b: v2(c.X + a.sin() * r1, c.Y - a.cos() * r1), w: if big { r * 0.035 } else { r * 0.012 }, c: ink(if big { 0.8 } else { 0.35 }) });
                }
                let [hh, mm, ss] = time;
                let hand = |deg: f32, len: f32, w: f32, col: D2D1_COLOR_F| {
                    let a = deg.to_radians();
                    ArtItem::Line { a: c, b: v2(c.X + a.sin() * len, c.Y - a.cos() * len), w, c: col }
                };
                items.push(hand(((hh % 12) as f32 + mm as f32 / 60.0) * 30.0, r * 0.5, r * 0.065, ink(1.0)));
                items.push(hand((mm as f32 + ss as f32 / 60.0) * 6.0, r * 0.75, r * 0.042, ink(1.0)));
                if secs.is_some() {
                    items.push(hand(ss as f32 * 6.0, r * 0.84, r * 0.018, acc));
                }
                items.push(ArtItem::Circle { c, r: r * 0.05, fill: Some(acc), stroke: None });
                (2.0 * r, 2.0 * r)
            }
        };
        Ok(Art { w, h, items })
    }

    /// Fasting, the next event and the Qibla as chips, cards or a strip.
    #[allow(clippy::too_many_arguments)]
    fn info_art(&self, style: &str, f: &Frame, label: char, rtl: bool, width: f32, tone: [f32; 3], acc: D2D1_COLOR_F, u: f32) -> Result<Art> {
        let [tr, tg, tb] = tone;
        let ink = |a: f32| rgba(tr, tg, tb, a);
        let mut items = vec![];
        let at = |mut p: Piece, x: f32, y: f32| {
            p.x = x;
            p.y = y;
            ArtItem::Text(p)
        };
        // a small icon for each kind, centred on (x, y) with radius r
        let qdeg = f.qibla.as_ref().map(|q| q.0).unwrap_or(0.0);
        let icon = |items: &mut Vec<ArtItem>, kind: u8, x: f32, y: f32, r: f32, col: D2D1_COLOR_F| match kind {
            0 => {
                // the sun on the horizon
                items.push(ArtItem::Arc { c: v2(x, y + r * 0.35), r: r * 0.62, a0: std::f32::consts::PI, a1: std::f32::consts::TAU, w: r * 0.18, col });
                items.push(ArtItem::Line { a: v2(x - r, y + r * 0.35), b: v2(x + r, y + r * 0.35), w: r * 0.18, c: col });
            }
            1 => {
                // a crescent
                items.push(ArtItem::Arc { c: v2(x, y), r: r * 0.72, a0: 0.55 * std::f32::consts::PI, a1: 1.45 * std::f32::consts::PI, w: r * 0.36, col });
            }
            _ => {
                // a compass pointing to the Qibla
                items.push(ArtItem::Circle { c: v2(x, y), r: r * 0.88, fill: None, stroke: Some((col, r * 0.14)) });
                let a = qdeg.to_radians();
                items.push(ArtItem::Line { a: v2(x, y), b: v2(x + a.sin() * r * 0.78, y - a.cos() * r * 0.78), w: r * 0.2, c: acc });
            }
        };
        let tiles = &f.tiles;
        let (w, h) = match style {
            "chips" => {
                // rounded tags in a row, wrapping when the column is narrow
                let fs = 0.92 * u;
                let (ch, gap, pad, ir) = (2.1 * u, 0.6 * u, 0.9 * u, 0.55 * u);
                let (mut x, mut y, mut maxw) = (0.0f32, 0.0f32, 0.0f32);
                for t in tiles {
                    let text = match t.kind {
                        0 => f.fasting.clone().unwrap_or_default(),
                        1 => f.event.clone().unwrap_or_default(),
                        _ => f.qibla.as_ref().map(|q| q.1.clone()).unwrap_or_default(),
                    };
                    let col = if t.accent { acc } else { ink(1.0) };
                    let p = self.label_piece(&text, label, fs, DWRITE_FONT_WEIGHT_NORMAL, rtl, 0, width, col)?;
                    let tw = self.width(&p)?;
                    let cw = pad + 2.0 * ir + 0.5 * u + tw + pad;
                    if x > 0.0 && x + cw > width {
                        x = 0.0;
                        y += ch + gap;
                    }
                    let (fill, stroke) = if t.accent { (rgba(acc.r, acc.g, acc.b, 0.12), rgba(acc.r, acc.g, acc.b, 0.45)) } else { (ink(0.08), ink(0.13)) };
                    items.push(ArtItem::Rect { r: rect(x, y, x + cw, y + ch), radius: ch / 2.0, fill, stroke: Some(stroke) });
                    icon(&mut items, t.kind, x + pad + ir, y + ch / 2.0, ir, col);
                    let ph = p.h;
                    items.push(at(p, x + pad + 2.0 * ir + 0.5 * u, y + (ch - ph) / 2.0));
                    x += cw + gap;
                    maxw = maxw.max(x - gap);
                }
                (maxw, y + ch)
            }
            "strip" => {
                // one ruled row of values, like the prayer times
                let n = tiles.len() as f32;
                let cell = width / n;
                let h = 3.6 * u;
                items.push(ArtItem::Line { a: v2(0.0, 0.0), b: v2(width, 0.0), w: 1.0, c: ink(0.2) });
                items.push(ArtItem::Line { a: v2(0.0, h), b: v2(width, h), w: 1.0, c: ink(0.2) });
                for (k, t) in tiles.iter().enumerate() {
                    let x = k as f32 * cell;
                    if k > 0 {
                        items.push(ArtItem::Line { a: v2(x, 0.55 * u), b: v2(x, h - 0.55 * u), w: 1.0, c: ink(0.2) });
                    }
                    let left = if k > 0 { x + 1.1 * u } else { x };
                    let lp = self.label_piece(&t.label.to_uppercase(), label, 0.68 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, rtl, 0, cell, ink(0.6))?;
                    let value = if t.kind == 1 { format!("{} {}", t.value, t.sub) } else { t.value.clone() };
                    let vp = self.label_piece(&value, label, 1.3 * u, DWRITE_FONT_WEIGHT_LIGHT, rtl, 0, cell, if t.accent { acc } else { ink(1.0) })?;
                    items.push(at(lp, left, 0.55 * u));
                    items.push(at(vp, left, 1.55 * u));
                }
                (width, h)
            }
            _ => {
                // small cards side by side; the fasting one is wider and shows how much of the fast is done
                let gap = 0.7 * u;
                let weights: Vec<f32> = tiles.iter().map(|t| if t.kind == 0 { 1.35 } else { 1.0 }).collect();
                let total: f32 = weights.iter().sum();
                let unit = (width - gap * (tiles.len() as f32 - 1.0)) / total;
                let pad = 0.9 * u;
                let h = 6.0 * u;
                let mut x = 0.0;
                for (t, wgt) in tiles.iter().zip(weights) {
                    let cw = unit * wgt;
                    items.push(ArtItem::Rect { r: rect(x, 0.0, x + cw, h), radius: 0.8 * u, fill: ink(0.07), stroke: Some(ink(0.13)) });
                    icon(&mut items, t.kind, x + pad + 0.4 * u, pad + 0.45 * u, 0.42 * u, ink(0.75));
                    let lp = self.label_piece(&t.label.to_uppercase(), label, 0.66 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, rtl, 0, cw - 2.0 * pad, ink(0.6))?;
                    let lh = lp.h;
                    items.push(at(lp, x + pad + 1.2 * u, pad + 0.45 * u - lh / 2.0));
                    let vp = self.label_piece(&t.value, label, 1.55 * u, DWRITE_FONT_WEIGHT_LIGHT, rtl, 0, cw - 2.0 * pad, if t.accent { acc } else { ink(1.0) })?;
                    items.push(at(vp, x + pad, pad + 1.15 * u));
                    let sp = self.label_piece(&t.sub, label, 0.78 * u, DWRITE_FONT_WEIGHT_NORMAL, rtl, 0, cw - 2.0 * pad, ink(0.65))?;
                    items.push(at(sp, x + pad, pad + 3.0 * u));
                    if let Some(pr) = t.progress {
                        let (bx, by, bw) = (x + pad, h - pad * 0.75, cw - 2.0 * pad);
                        items.push(ArtItem::Rect { r: rect(bx, by, bx + bw, by + 0.2 * u), radius: 0.1 * u, fill: ink(0.15), stroke: None });
                        items.push(ArtItem::Rect { r: rect(bx, by, bx + bw * pr.clamp(0.0, 1.0), by + 0.2 * u), radius: 0.1 * u, fill: acc, stroke: None });
                    }
                    x += cw + gap;
                }
                (width, h)
            }
        };
        Ok(Art { w, h, items })
    }

    /// Draws an `Art` with its top-left corner at (ox, oy); text goes through `text` so it gets
    /// the same shadow as the rest of the panel.
    fn draw_art(&self, t: &ID2D1RenderTarget, a: &Art, ox: f32, oy: f32, text: &dyn Fn(&Piece) -> Result<()>) -> Result<()> {
        let mv = |p: Vector2| v2(p.X + ox, p.Y + oy);
        unsafe {
            for it in &a.items {
                match it {
                    ArtItem::Text(p) => text(&Piece { layout: p.layout.clone(), x: p.x + ox, y: p.y + oy, w: p.w, h: p.h, color: p.color })?,
                    ArtItem::Rect { r, radius, fill, stroke } => {
                        let rr = D2D1_ROUNDED_RECT { rect: rect(r.left + ox, r.top + oy, r.right + ox, r.bottom + oy), radiusX: *radius, radiusY: *radius };
                        t.FillRoundedRectangle(&rr, &t.CreateSolidColorBrush(fill, None)?);
                        if let Some(s) = stroke {
                            t.DrawRoundedRectangle(&rr, &t.CreateSolidColorBrush(s, None)?, 1.0, None);
                        }
                    }
                    ArtItem::Line { a, b, w, c } => t.DrawLine(mv(*a), mv(*b), &t.CreateSolidColorBrush(c, None)?, *w, None),
                    ArtItem::Circle { c, r, fill, stroke } => {
                        let e = D2D1_ELLIPSE { point: mv(*c), radiusX: *r, radiusY: *r };
                        if let Some(fc) = fill {
                            t.FillEllipse(&e, &t.CreateSolidColorBrush(fc, None)?);
                        }
                        if let Some((sc, w)) = stroke {
                            t.DrawEllipse(&e, &t.CreateSolidColorBrush(sc, None)?, *w, None);
                        }
                    }
                    ArtItem::Arc { c, r, a0, a1, w, col } => {
                        let c = mv(*c);
                        let geo = self.factory.CreatePathGeometry()?;
                        let sink = geo.Open()?;
                        sink.BeginFigure(v2(c.X + r * a0.cos(), c.Y + r * a0.sin()), D2D1_FIGURE_BEGIN_HOLLOW);
                        sink.AddArc(&D2D1_ARC_SEGMENT {
                            point: v2(c.X + r * a1.cos(), c.Y + r * a1.sin()),
                            size: D2D_SIZE_F { width: *r, height: *r },
                            rotationAngle: 0.0,
                            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                            arcSize: if a1 - a0 > std::f32::consts::PI { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
                        });
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        sink.Close()?;
                        t.DrawGeometry(&geo, &t.CreateSolidColorBrush(col, None)?, *w, None);
                    }
                }
            }
        }
        Ok(())
    }

    /// How wide the laid-out text actually is.
    fn width(&self, p: &Piece) -> Result<f32> {
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe { p.layout.GetMetrics(&mut m)? };
        Ok(m.width)
    }

    /// Like `piece`, but shrinks the font (down to 60%) until the text fits `max_h`, and as a
    /// last resort ends it with an ellipsis rather than overflow.
    #[allow(clippy::too_many_arguments)]
    fn fitted(&self, text: &str, which: char, size: f32, weight: DWRITE_FONT_WEIGHT, rtl: bool, width: f32, max_h: f32, align: u8, color: D2D1_COLOR_F) -> Result<Piece> {
        let mut s = size;
        loop {
            let p = self.piece(text, &self.format(which, s, weight, rtl)?, width, align, 0.0, color)?;
            if p.h <= max_h {
                return Ok(p);
            }
            if s <= size * 0.6 {
                unsafe {
                    p.layout.SetMaxHeight(max_h)?;
                    let sign = self.dwrite.CreateEllipsisTrimmingSign(&p.layout)?;
                    let trim = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_WORD, delimiter: 0, delimiterCount: 0 };
                    p.layout.SetTrimming(&trim, &sign)?;
                }
                return Ok(Piece { h: max_h, ..p });
            }
            s *= 0.92;
        }
    }

    /// Paints `area` (client coordinates) of the window covering all `monitors`.
    /// `panels`: the monitors (indexes into `monitors`) that show the text panel.
    /// `work`: each monitor without its taskbar, where the text panel is laid out.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(&self, hdc: HDC, area: RECT, monitors: &[RECT], work: &[RECT], panels: &[usize], f: &Frame, st: &Style) -> Result<()> {
        unsafe {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_IGNORE },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let t = self.factory.CreateDCRenderTarget(&props)?;
            t.BindDC(hdc, &area)?;
            t.BeginDraw();
            let base = Matrix3x2::translation(-area.left as f32, -area.top as f32);
            t.SetTransform(&base);
            t.Clear(Some(&rgba(0.04, 0.05, 0.07, 1.0)));
            let rt: ID2D1RenderTarget = t.cast()?;
            for (i, m) in monitors.iter().enumerate() {
                if m.right <= area.left || m.left >= area.right || m.bottom <= area.top || m.top >= area.bottom {
                    continue;
                }
                self.picture(&rt, i, *m, f, &base)?;
                if panels.contains(&i) {
                    self.panel(&rt, i, work.get(i).copied().unwrap_or(*m), f, st)?;
                }
            }
            t.EndDraw(None, None)?;
        }
        Ok(())
    }

    /// The photo with the drift zoom and the light of the hour.
    fn picture(&self, t: &ID2D1RenderTarget, i: usize, m: RECT, f: &Frame, base: &Matrix3x2) -> Result<()> {
        unsafe {
            let photo = t.CreateBitmapFromWicBitmap(&self.photos[self.of_monitor.get(i).copied().unwrap_or(0)], None)?;
            let dst = rect(m.left as f32, m.top as f32, m.right as f32, m.bottom as f32);
            let c = Vector2 { X: (m.left + m.right) as f32 / 2.0, Y: (m.top + m.bottom) as f32 / 2.0 };
            t.PushAxisAlignedClip(&dst, D2D1_ANTIALIAS_MODE_ALIASED);
            t.SetTransform(&(Matrix3x2::scale_around(f.zoom, f.zoom, c) * base));
            t.DrawBitmap(&photo, Some(&dst), 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
            t.SetTransform(base);
            if f.light.dark > 0.001 {
                t.FillRectangle(&dst, &t.CreateSolidColorBrush(&rgba(0.0, 0.01, 0.03, f.light.dark), None)?);
            }
            let tint = f.light.tint;
            if tint[3] > 0.001 {
                t.FillRectangle(&dst, &t.CreateSolidColorBrush(&rgba(tint[0], tint[1], tint[2], tint[3]), None)?);
            }
            t.PopAxisAlignedClip();
        }
        Ok(())
    }

    /// The text: clock group, reminder group, prayer row and system strip.
    fn panel(&self, t: &ID2D1RenderTarget, mi: usize, m: RECT, f: &Frame, st: &Style) -> Result<()> {
        let (x0, y0) = (m.left as f32, m.top as f32);
        let (w, h) = ((m.right - m.left) as f32, (m.bottom - m.top) as f32);
        let u = w.min(h * 16.0 / 9.0) / 100.0 * st.scale;
        let [tr, tg, tb] = st.tone;
        let white = rgba(tr, tg, tb, 1.0);
        let soft = rgba(tr, tg, tb, 0.76);
        let faint = rgba(tr, tg, tb, 0.55);
        let acc = rgba(st.accent[0], st.accent[1], st.accent[2], 1.0);
        let label = if st.rtl_labels { 'a' } else if st.bn_labels { 'b' } else { 'e' };
        let track = if label == 'e' { 1.0 } else { 0.0 };
        let bar = st.layout == "bar";
        let corner = st.layout == "corner";
        let centre = st.layout == "centre";

        // Column for each layout: left, right, alignment, top of the clock, bottom of the stack.
        let (col_l, col_r, align, top, bottom, clock_size) = match st.layout.as_str() {
            "centre" => (x0 + w * 0.25, x0 + w * 0.75, 1u8, y0 + h * 0.14, y0 + h - 3.6 * u, 9.6 * u),
            "corner" => {
                let (l, r) = if st.side_left { (x0 + 3.6 * u, x0 + 33.0 * u) } else { (x0 + w - 33.0 * u, x0 + w - 3.6 * u) };
                (l, r, if st.side_left { 0 } else { 2 }, 0.0, y0 + h - 3.4 * u, 5.6 * u)
            }
            "bar" => (x0 + 4.0 * u, x0 + w - 4.0 * u, 0, 0.0, y0 + h - 2.4 * u, 5.0 * u),
            _ => {
                let cw = st.col_w;
                let (l, r) = if st.side_left { (x0 + 3.2 * u, x0 + (3.2 + cw) * u) } else { (x0 + w - (3.2 + cw) * u, x0 + w - 3.2 * u) };
                (l, r, 0, y0 + 3.2 * u, y0 + h - 3.2 * u, 8.4 * u)
            }
        };
        let cw = col_r - col_l;

        // In the bar the screen is split in three: clock | reminder | prayer row.
        let (clock_x, clock_w) = if bar { (col_l, cw * 0.25) } else { (col_l, cw) };
        let (rem_x, rem_w) = if bar { (col_l + cw * 0.28, cw * 0.4) } else { (col_l, cw) };
        let (row_x, row_w) = if bar { (col_l + cw * 0.71, cw * 0.29) } else { (col_l, cw) };

        // ---- clock group ----
        let clock_font = 'c';
        let clock_weight = match st.clock_style.as_str() {
            "bold" => DWRITE_FONT_WEIGHT_SEMI_BOLD,
            "light" => DWRITE_FONT_WEIGHT_LIGHT,
            "serif" => DWRITE_FONT_WEIGHT_NORMAL,
            _ => DWRITE_FONT_WEIGHT_EXTRA_LIGHT,
        };
        let clock_size = clock_size * st.clock_scale;
        let caps = |t: &str| if st.caps { t.to_uppercase() } else { t.to_string() };
        let mut clock_text = f.clock.clone();
        if let Some(s) = &f.seconds {
            clock_text.push_str(s);
        }
        let has_next = st.show_next && !f.next.is_empty();
        let design = st.clock_design.as_str();
        let (hours, minutes) = f.clock.split_once(':').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((f.clock.clone(), String::new()));
        let secs = f.seconds.as_ref().map(|s| s.trim_start_matches(':').to_string());
        let bold = if design == "split" { hours.encode_utf16().count() } else { 0 };
        // Arabic-Indic digits have no tabular feature in every font; the fixed size still holds.
        let (clock_piece, clock_full_w) = self.clock_line(&clock_text, clock_font, clock_size, clock_weight, clock_w, align, white, bold)?;
        let art = if st.show_clock && matches!(design, "stack" | "tiles" | "pill" | "dial") {
            Some(self.clock_art(design, &hours, &minutes, secs.as_deref(), &f.ampm, f.time, clock_size, clock_weight, clock_w, [tr, tg, tb], acc, u)?)
        } else {
            None
        };
        let mut tops = vec![(self.piece(&caps(&f.meta), &self.format(label, 0.95 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, st.rtl_labels)?, clock_w, align, 0.12 * u * track * if st.caps { 1.0 } else { 0.0 }, faint)?, 0.2 * u)];
        let clock_idx = st.show_clock.then_some(1);
        if st.show_clock {
            match &art {
                // an empty piece that only holds the drawing's place
                Some(a) => tops.push((self.blank(a.h, clock_w)?, 0.7 * u)),
                None => tops.push((clock_piece, -0.4 * u)),
            }
        }
        // Extra lines under the countdown: fasting times, the next event and the Qibla.
        let small = if corner || bar { 0.85 * u } else { 0.95 * u };
        let mut extra: Vec<(Piece, bool)> = vec![]; // (piece, is the qibla line)
        let info_style = if bar || f.tiles.is_empty() { "lines" } else { st.info_style.as_str() };
        let info = if info_style == "lines" { None } else { Some(self.info_art(info_style, f, label, st.rtl_labels, clock_w, [tr, tg, tb], acc, u)?) };
        if let Some(a) = &info {
            extra.push((self.blank(a.h, clock_w)?, false));
        } else if let Some(t) = &f.fasting {
            extra.push((self.piece(t, &self.format(label, small * 1.05, DWRITE_FONT_WEIGHT_NORMAL, st.rtl_labels)?, clock_w, align, 0.0, acc)?, false));
        }
        if let (None, Some(t)) = (&info, &f.event) {
            extra.push((self.piece(t, &self.format(label, small, DWRITE_FONT_WEIGHT_NORMAL, st.rtl_labels)?, clock_w, align, 0.0, soft)?, false));
        }
        if let (None, Some((_, t))) = (&info, &f.qibla) {
            // room on the left for the little compass
            let indent = if align == 0 { 1.5 * u } else { 0.0 };
            let mut p = self.piece(t, &self.format(label, small, DWRITE_FONT_WEIGHT_NORMAL, st.rtl_labels)?, clock_w - indent, align, 0.0, soft)?;
            p.w += indent;
            extra.push((p, true));
        }
        let next_idx = tops.len();
        if has_next {
            let size = if corner || bar { 1.15 * u } else { 1.35 * u } * if st.show_clock { 1.0 } else { 1.5 };
            let below = if extra.is_empty() { 0.0 } else if matches!(st.progress_style.as_str(), "line" | "dots") { 1.3 * u } else { 0.5 * u };
            tops.push((self.piece(&f.next, &self.format(label, size, DWRITE_FONT_WEIGHT_LIGHT, st.rtl_labels)?, clock_w, align, 0.0, white)?, below));
        }
        let first_extra = tops.len();
        let info_idx = info.as_ref().map(|_| first_extra);
        let qibla_idx = extra.iter().position(|(_, q)| *q).map(|i| first_extra + i);
        let n_extra = extra.len();
        for (k, (p, _)) in extra.into_iter().enumerate() {
            tops.push((p, if k + 1 < n_extra { 0.25 * u } else { 0.0 }));
        }
        let ampm = self.piece(&f.ampm, &self.format(label, clock_size * 0.17, DWRITE_FONT_WEIGHT_LIGHT, false)?, 8.0 * u, 0, 0.0, soft)?;

        // ---- heights of the prayer times and the system strip ----
        // The bottom bar is short: a tall list becomes cards there.
        let times_style = if bar && st.times_style == "list" { "grid" } else { st.times_style.as_str() };
        let row_h = if st.show_times && !f.times.is_empty() {
            match times_style {
                "grid" => 7.4 * u,
                "list" => 6.0 * 1.85 * u + 0.6 * u,
                "timeline" => 5.0 * u,
                _ => 4.0 * u,
            }
        } else {
            0.0
        };
        let strip_h = if f.monitor.is_some() {
            match st.monitor_style.as_str() {
                "bars" => 3.0 * u,
                "rings" => 6.2 * u,
                "text" => 1.4 * u,
                _ => 3.4 * u,
            }
        } else {
            0.0
        };
        // ---- reminder group ----
        // If the column is too short for everything, squeeze the reminder step by step: smaller
        // Arabic and meaning, then no special-day line, then no meaning. Nothing may overlap.
        let gaps = if st.behind == "glass" { 4.8 * u } else { 3.0 * u } + if row_h > 0.0 { 1.6 * u } else { 0.0 } + if strip_h > 0.0 { 1.4 * u } else { 0.0 };
        let clock_h: f32 = tops.iter().map(|(p, g)| p.h + g).sum();
        let budget = if corner || bar { f32::MAX } else { (bottom - top) - clock_h - row_h - strip_h - gaps };
        let mut rems: Vec<(Piece, f32)> = vec![]; // (piece, gap below)
        for level in 0..4 {
            rems.clear();
            let shrink = if level >= 1 { 0.65 } else { 1.0 };
            if st.show_special && level < 2 {
                if let Some((text, reference)) = &f.special {
                    rems.push((self.piece(text, &self.format(label, 0.95 * u, DWRITE_FONT_WEIGHT_NORMAL, st.rtl_labels)?, rem_w, align, 0.0, acc)?, 0.15 * u));
                    rems.push((self.piece(reference, &self.format('e', 0.72 * u, DWRITE_FONT_WEIGHT_NORMAL, false)?, rem_w, align, 0.0, faint)?, 1.3 * u));
                }
            }
            if st.show_reminder && !f.kind.is_empty() {
                rems.push((self.piece(&caps(&f.kind), &self.format(label, 0.8 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, st.rtl_labels)?, rem_w, align, 0.1 * u * track * if st.caps { 1.0 } else { 0.0 }, acc)?, 0.45 * u));
                if !f.arabic.is_empty() {
                    let max = if bar { 5.2 * u } else { 7.5 * u } * shrink;
                    rems.push((self.fitted(&f.arabic, 'a', 2.3 * u * st.ar_scale, DWRITE_FONT_WEIGHT_NORMAL, true, rem_w, max, if st.ar_right { 2 } else { align }, white)?, 0.45 * u));
                }
                if !f.meaning.is_empty() && level < 3 {
                    let which = if f.meaning_bn { 'b' } else { 'e' };
                    let max = if bar { 3.2 * u } else { 5.0 * u } * shrink;
                    rems.push((self.fitted(&f.meaning, which, 1.05 * u, DWRITE_FONT_WEIGHT_LIGHT, false, rem_w, max, align, soft)?, 0.4 * u));
                }
                rems.push((self.piece(&f.source, &self.format('e', 0.78 * u, DWRITE_FONT_WEIGHT_NORMAL, false)?, rem_w, align, 0.0, faint)?, 0.0));
            }
            if rems.iter().map(|(p, g)| p.h + g).sum::<f32>() <= budget {
                break;
            }
        }

        // ---- vertical placement ----
        let mut y = bottom;
        let mut strip_y = y - strip_h;
        if strip_h > 0.0 {
            y = strip_y - 1.4 * u;
        }
        let mut row_y = y - row_h;
        let mut rem_bottom = if bar { y } else if row_h > 0.0 { row_y - 1.6 * u } else { y };
        if bar && strip_h > 0.0 {
            rem_bottom = strip_y - 1.4 * u;
        }
        let mut yy = rem_bottom;
        let mut placed: Vec<Piece> = vec![];
        for (mut p, gap) in rems.into_iter().rev() {
            yy -= p.h + gap;
            p.x = rem_x;
            p.y = yy;
            placed.push(p);
        }
        let mut rem_top = placed.last().map(|p| p.y).unwrap_or(rem_bottom);

        let span: f32 = tops.iter().map(|(p, g)| p.h + g).sum();
        // Space between the clock group and the reminder. Glass cards add 1.6u padding around each
        // group, so they need more room or the two cards touch.
        let group_gap = match (st.behind.as_str(), corner) {
            ("glass", _) => 3.2 * u + 1.6 * u,
            (_, true) => 2.0 * u,
            _ => 3.0 * u,
        };
        let mut cy = if corner {
            rem_top - group_gap - span
        } else if bar {
            rem_bottom - span
        } else {
            top
        };
        // Together: close the gap between the clock and the rest, and centre the whole block.
        if st.together && !corner && !bar {
            let gap = group_gap;
            let block = span + gap + (bottom - rem_top);
            let free = (bottom - top) - block;
            if free > 0.0 {
                cy = top + free / 2.0;
                let lift = free / 2.0;
                for p in placed.iter_mut() {
                    p.y -= lift;
                }
                row_y -= lift;
                strip_y -= lift;
                rem_top -= lift;
            }
        }
        let mut clock_pieces: Vec<Piece> = vec![];
        for (k, (mut p, gap)) in tops.into_iter().enumerate() {
            p.x = clock_x;
            if Some(k) == qibla_idx && align == 0 {
                p.x += 1.5 * u;
                p.w -= 1.5 * u;
            }
            p.y = cy;
            cy += p.h + gap;
            clock_pieces.push(p);
        }

        // ---- what sits behind the text ----
        let a = st.shade;
        unsafe {
            match st.behind.as_str() {
                "glass" => {
                    let pad = 1.6 * u;
                    let mut boxes = vec![];
                    if let Some(r) = bounds(&clock_pieces.iter().collect::<Vec<_>>(), pad) {
                        boxes.push(r);
                    }
                    let mut lower: Vec<&Piece> = placed.iter().collect();
                    let filler;
                    if row_h > 0.0 || strip_h > 0.0 {
                        let ry = if row_h > 0.0 { row_y } else { strip_y };
                        let (fx, fw) = if bar { (row_x, row_w) } else { (col_l, cw) };
                        filler = Piece { layout: clock_pieces[0].layout.clone(), x: fx, y: ry, w: fw, h: bottom - ry, color: white };
                        lower.push(&filler);
                    }
                    if bar {
                        // one card per column in the bar
                        for p in lower {
                            boxes.push(rect(p.x - pad, p.y - pad, p.x + p.w + pad, p.y + p.h + pad));
                        }
                    } else if let Some(r) = bounds(&lower, pad) {
                        boxes.push(r);
                    }
                    let soft_bmp = match self.blurred.get(self.of_monitor.get(mi).copied().unwrap_or(0)) {
                        Some(b) => Some(t.CreateBitmapFromWicBitmap(b, None)?),
                        None => None,
                    };
                    for b in boxes {
                        let rr = D2D1_ROUNDED_RECT { rect: b, radiusX: st.corner_r * u, radiusY: st.corner_r * u };
                        if let Some(bmp) = &soft_bmp {
                            let brush = t.CreateBitmapBrush(bmp, None, None)?;
                            let sz = bmp.GetPixelSize();
                            brush.SetTransform(&(Matrix3x2::scale(w / sz.width.max(1) as f32, h / sz.height.max(1) as f32) * Matrix3x2::translation(x0, y0)));
                            t.FillRoundedRectangle(&rr, &brush);
                        }
                        // Light glass is a frosted white veil, still with enough shade under it for
                        // white text to stay readable on bright pictures.
                        if st.glass_light {
                            t.FillRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.12), None)?);
                            t.FillRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(0.10, 0.10, 0.12, 0.22 + 0.3 * a), None)?);
                        } else {
                            t.FillRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(0.03, 0.04, 0.07, 0.2 + 0.45 * a), None)?);
                        }
                        t.DrawRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, if st.glass_light { 0.26 } else { 0.14 }), None)?, 1.0, None);
                    }
                }
                "none" => {}
                _ => {
                    let dark = |al: f32| rgba(0.02, 0.03, 0.05, (al * (0.25 + a)).min(1.0));
                    let stops = [
                        D2D1_GRADIENT_STOP { position: 0.0, color: dark(0.0) },
                        D2D1_GRADIENT_STOP { position: 0.5, color: dark(0.62) },
                        D2D1_GRADIENT_STOP { position: 0.78, color: dark(0.95) },
                        D2D1_GRADIENT_STOP { position: 1.0, color: dark(1.15) },
                    ];
                    let coll = t.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
                    let v = |x: f32, y: f32| Vector2 { X: x, Y: y };
                    let (start, end, fill) = if centre || bar {
                        let from = if bar { y0 + h - 28.0 * u } else { y0 + h * 0.45 };
                        (v(x0, from), v(x0, y0 + h), rect(x0, from, x0 + w, y0 + h))
                    } else if corner {
                        // A soft round shade growing out of the corner: no edges anywhere.
                        let cx = if st.side_left { x0 } else { x0 + w };
                        let r = (y0 + h - rem_top + 18.0 * u).max(40.0 * u) * 1.25;
                        let stops = [
                            D2D1_GRADIENT_STOP { position: 0.0, color: dark(1.0) },
                            D2D1_GRADIENT_STOP { position: 0.55, color: dark(0.75) },
                            D2D1_GRADIENT_STOP { position: 1.0, color: dark(0.0) },
                        ];
                        let coll = t.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
                        let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES { center: v(cx, y0 + h), gradientOriginOffset: v(0.0, 0.0), radiusX: r * 1.15, radiusY: r };
                        let brush = t.CreateRadialGradientBrush(&props, None, &coll)?;
                        t.FillRectangle(&rect(x0, y0, x0 + w, y0 + h), &brush);
                        (v(0.0, 0.0), v(0.0, 0.0), rect(0.0, 0.0, 0.0, 0.0))
                    } else if st.side_left {
                        (v(x0 + w * 0.6, y0), v(x0, y0), rect(x0, y0, x0 + w * 0.6, y0 + h))
                    } else {
                        (v(x0 + w * 0.4, y0), v(x0 + w, y0), rect(x0 + w * 0.4, y0, x0 + w, y0 + h))
                    };
                    let brush = t.CreateLinearGradientBrush(&D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: start, endPoint: end }, None, &coll)?;
                    t.FillRectangle(&fill, &brush);
                    if centre {
                        // a lighter wash at the top, behind the clock
                        let stops = [D2D1_GRADIENT_STOP { position: 0.0, color: dark(0.7) }, D2D1_GRADIENT_STOP { position: 1.0, color: dark(0.0) }];
                        let coll = t.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
                        let brush = t.CreateLinearGradientBrush(&D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: v(x0, y0), endPoint: v(x0, y0 + h * 0.5) }, None, &coll)?;
                        t.FillRectangle(&rect(x0, y0, x0 + w, y0 + h * 0.5), &brush);
                    }
                }
            }
        }

        // ---- text ----
        let shadow = st.shadow;
        let draw = |p: &Piece| -> Result<()> {
            unsafe {
                if shadow > 0 {
                    let s = t.CreateSolidColorBrush(&rgba(0.0, 0.0, 0.0, if shadow > 1 { 0.9 } else { 0.6 } * p.color.a), None)?;
                    t.DrawTextLayout(Vector2 { X: p.x + 0.08 * u, Y: p.y + 0.1 * u }, &p.layout, &s, D2D1_DRAW_TEXT_OPTIONS_NONE);
                    if shadow > 1 {
                        let wide = t.CreateSolidColorBrush(&rgba(0.0, 0.0, 0.0, 0.35 * p.color.a), None)?;
                        t.DrawTextLayout(Vector2 { X: p.x + 0.16 * u, Y: p.y + 0.2 * u }, &p.layout, &wide, D2D1_DRAW_TEXT_OPTIONS_NONE);
                    }
                }
                t.DrawTextLayout(Vector2 { X: p.x, Y: p.y }, &p.layout, &t.CreateSolidColorBrush(&p.color, None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
            }
            Ok(())
        };
        for p in &clock_pieces {
            draw(p)?;
        }
        if let (Some(ci), Some(a)) = (clock_idx, &art) {
            let c = &clock_pieces[ci];
            let ox = c.x + match align {
                1 => (c.w - a.w) / 2.0,
                2 => c.w - a.w,
                _ => 0.0,
            };
            self.draw_art(t, a, ox, c.y, &draw)?;
            self.clock_rect.borrow_mut().insert(mi, RECT { left: (ox - u) as i32, top: (c.y - u) as i32, right: (ox + a.w + u) as i32, bottom: (c.y + a.h + u) as i32 });
        } else if !st.show_clock {
            self.clock_rect.borrow_mut().remove(&mi);
        }
        if let (Some(ii), Some(a)) = (info_idx, &info) {
            let c = &clock_pieces[ii];
            let ox = c.x + match align {
                1 => (c.w - a.w) / 2.0,
                2 => c.w - a.w,
                _ => 0.0,
            };
            self.draw_art(t, a, ox, c.y, &draw)?;
        }
        if let (Some(ci), None) = (clock_idx, &art) {
            unsafe {
            // am/pm sits right after the clock digits; remember the clock's box for seconds.
            let c = &clock_pieces[ci];
            let mut cm = DWRITE_TEXT_METRICS::default();
            c.layout.GetMetrics(&mut cm)?;
            // Use the widest possible time, not this second's, so am/pm never shifts.
            let dl = match align {
                1 => c.x + (c.w - clock_full_w) / 2.0,
                2 => c.x + c.w - clock_full_w,
                _ => c.x + cm.left,
            };
            let dr = dl + clock_full_w.max(cm.width);
            if !f.ampm.is_empty() {
                let mut ap = ampm;
                ap.x = dr + 0.6 * u;
                ap.y = c.y + c.h * 0.66 - ap.h;
                draw(&ap)?;
            }
            self.clock_rect.borrow_mut().insert(mi, RECT { left: (dl - u) as i32, top: c.y as i32, right: (dr + 7.0 * u) as i32, bottom: (c.y + c.h) as i32 });
            }
        }
        if has_next && st.progress_style != "off" {
            // progress from the last prayer to the next one
            let n = &clock_pieces[next_idx];
            let frac = f.progress.clamp(0.0, 1.0);
            let ly = n.y + n.h + 0.55 * u;
            let len = if centre { cw * 0.3 } else { clock_w.min(16.0 * u) };
            let lx = match align {
                1 => clock_x + (clock_w - len) / 2.0,
                2 => clock_x + clock_w - len,
                _ => clock_x,
            };
            unsafe {
                let track = t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.18), None)?;
                let fill = t.CreateSolidColorBrush(&acc, None)?;
                match st.progress_style.as_str() {
                    "dots" => {
                        let count = 8;
                        let step = len / count as f32;
                        let lit = (frac * count as f32).round() as usize;
                        for k in 0..count {
                            let e = D2D1_ELLIPSE { point: Vector2 { X: lx + step * (k as f32 + 0.5), Y: ly + 0.1 * u }, radiusX: 0.22 * u, radiusY: 0.22 * u };
                            t.FillEllipse(&e, if k < lit { &fill } else { &track });
                        }
                    }
                    "ring" => {
                        // a small ring just after the countdown text
                        let mut m = DWRITE_TEXT_METRICS::default();
                        n.layout.GetMetrics(&mut m)?;
                        let r = 0.55 * u;
                        let (cx, cy) = (n.x + m.left + m.width + 1.2 * u, n.y + n.h / 2.0);
                        t.DrawEllipse(&D2D1_ELLIPSE { point: Vector2 { X: cx, Y: cy }, radiusX: r, radiusY: r }, &track, 0.2 * u, None);
                        let v = frac.clamp(0.005, 0.999);
                        let a0 = -std::f32::consts::FRAC_PI_2;
                        let a1 = a0 + v * std::f32::consts::TAU;
                        let geo = self.factory.CreatePathGeometry()?;
                        let sink = geo.Open()?;
                        sink.BeginFigure(Vector2 { X: cx + r * a0.cos(), Y: cy + r * a0.sin() }, D2D1_FIGURE_BEGIN_HOLLOW);
                        sink.AddArc(&D2D1_ARC_SEGMENT {
                            point: Vector2 { X: cx + r * a1.cos(), Y: cy + r * a1.sin() },
                            size: D2D_SIZE_F { width: r, height: r },
                            rotationAngle: 0.0,
                            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                            arcSize: if v > 0.5 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
                        });
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        sink.Close()?;
                        t.DrawGeometry(&geo, &fill, 0.2 * u, None);
                    }
                    _ => {
                        t.FillRectangle(&rect(lx, ly, lx + len, ly + 0.14 * u), &track);
                        t.FillRectangle(&rect(lx, ly, lx + len * frac, ly + 0.14 * u), &fill);
                    }
                }
            }
        }
        if let (Some(qi), Some((deg, _))) = (qibla_idx, &f.qibla) {
            // a little compass whose needle points to the Qibla (north is up)
            let p = &clock_pieces[qi];
            unsafe {
                let mut m = DWRITE_TEXT_METRICS::default();
                p.layout.GetMetrics(&mut m)?;
                let r = 0.5 * u;
                let (cx, cy) = (p.x + m.left - 0.85 * u, p.y + p.h / 2.0);
                let ring = t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.45), None)?;
                t.DrawEllipse(&D2D1_ELLIPSE { point: Vector2 { X: cx, Y: cy }, radiusX: r, radiusY: r }, &ring, 0.09 * u, None);
                let a = deg.to_radians();
                let tip = Vector2 { X: cx + 0.85 * r * a.sin(), Y: cy - 0.85 * r * a.cos() };
                t.DrawLine(Vector2 { X: cx, Y: cy }, tip, &t.CreateSolidColorBrush(&acc, None)?, 0.12 * u, None);
                t.FillEllipse(&D2D1_ELLIPSE { point: tip, radiusX: 0.1 * u, radiusY: 0.1 * u }, &t.CreateSolidColorBrush(&acc, None)?);
            }
        }
        for p in &placed {
            draw(p)?;
        }

        // ---- prayer times ----
        if row_h > 0.0 {
            let n = f.times.len();
            let order: Vec<usize> = if st.rtl_labels { (0..n).rev().collect() } else { (0..n).collect() };
            let ta = if centre { 1 } else { 0 };
            let lf = self.format(label, 0.72 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, false)?;
            let vf = self.format('e', 1.15 * u, DWRITE_FONT_WEIGHT_LIGHT, false)?;
            let fill = |r: D2D_RECT_F, c: D2D1_COLOR_F, rad: f32| -> Result<()> {
                unsafe { t.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r, radiusX: rad, radiusY: rad }, &t.CreateSolidColorBrush(&c, None)?) };
                Ok(())
            };
            match times_style {
                "grid" => {
                    // 3 × 2 cards; the current prayer's card is tinted with the accent
                    let (gx, gy) = (0.6 * u, 0.6 * u);
                    let cwid = (row_w - 2.0 * gx) / 3.0;
                    let ch = (row_h - gy) / 2.0;
                    for (slot, &i) in order.iter().enumerate() {
                        let (name, time, now) = &f.times[i];
                        let x = row_x + (slot % 3) as f32 * (cwid + gx);
                        let y = row_y + (slot / 3) as f32 * (ch + gy);
                        fill(rect(x, y, x + cwid, y + ch), if *now { rgba(acc.r, acc.g, acc.b, 0.24) } else { rgba(1.0, 1.0, 1.0, 0.07) }, 0.6 * u)?;
                        let mut a = self.piece(name, &lf, cwid - 1.6 * u, 0, 0.0, if *now { acc } else { faint })?;
                        a.x = x + 0.8 * u;
                        a.y = y + 0.55 * u;
                        draw(&a)?;
                        let mut b = self.piece(time, &vf, cwid - 1.6 * u, 0, 0.0, if *now { white } else { soft })?;
                        b.x = x + 0.8 * u;
                        b.y = y + 1.55 * u;
                        draw(&b)?;
                    }
                }
                "list" => {
                    // one prayer per line: name on one side, time on the other
                    let lh = 1.85 * u;
                    let nf = self.format(label, 0.95 * u, DWRITE_FONT_WEIGHT_NORMAL, st.rtl_labels)?;
                    let tf = self.format('e', 1.0 * u, DWRITE_FONT_WEIGHT_NORMAL, false)?;
                    for (k, i) in (0..n).enumerate() {
                        let (name, time, now) = &f.times[i];
                        let y = row_y + k as f32 * lh;
                        if *now {
                            fill(rect(row_x - 0.6 * u, y + 0.1 * u, row_x + row_w + 0.6 * u, y + lh - 0.1 * u), rgba(acc.r, acc.g, acc.b, 0.16), 0.4 * u)?;
                        }
                        let mut a = self.piece(name, &nf, row_w, if st.rtl_labels { 2 } else { 0 }, 0.0, if *now { acc } else { soft })?;
                        a.x = row_x;
                        a.y = y + (lh - a.h) / 2.0;
                        draw(&a)?;
                        let mut b = self.piece(time, &tf, row_w, if st.rtl_labels { 0 } else { 2 }, 0.0, if *now { white } else { soft })?;
                        b.x = row_x;
                        b.y = y + (lh - b.h) / 2.0;
                        draw(&b)?;
                        if k + 1 < n && st.dividers {
                            fill(rect(row_x, y + lh - 0.5, row_x + row_w, y + lh), rgba(1.0, 1.0, 1.0, 0.07), 0.0)?;
                        }
                    }
                }
                "timeline" => {
                    // a line with a dot per prayer; the current one glows
                    let colw = row_w / n as f32;
                    let ly = row_y + 1.0 * u;
                    fill(rect(row_x, ly - 0.06 * u, row_x + row_w, ly + 0.06 * u), rgba(1.0, 1.0, 1.0, 0.2), 0.0)?;
                    for (slot, &i) in order.iter().enumerate() {
                        let (name, time, now) = &f.times[i];
                        let x = row_x + slot as f32 * colw;
                        unsafe {
                            let e = |r: f32| D2D1_ELLIPSE { point: Vector2 { X: x + 0.3 * u, Y: ly }, radiusX: r, radiusY: r };
                            if *now {
                                t.FillEllipse(&e(0.65 * u), &t.CreateSolidColorBrush(&rgba(acc.r, acc.g, acc.b, 0.3), None)?);
                                t.FillEllipse(&e(0.32 * u), &t.CreateSolidColorBrush(&acc, None)?);
                            } else {
                                t.FillEllipse(&e(0.26 * u), &t.CreateSolidColorBrush(&rgba(0.8, 0.8, 0.8, 0.9), None)?);
                            }
                        }
                        let mut a = self.piece(name, &lf, colw, 0, 0.0, if *now { acc } else { faint })?;
                        a.x = x;
                        a.y = ly + 1.0 * u;
                        draw(&a)?;
                        let mut b = self.piece(time, &vf, colw, 0, 0.0, if *now { white } else { soft })?;
                        b.x = x;
                        b.y = ly + 2.1 * u;
                        draw(&b)?;
                    }
                }
                _ => {
                    let colw = row_w / n as f32;
                    if st.dividers {
                        fill(rect(row_x, row_y, row_x + row_w, row_y + 1.0), rgba(1.0, 1.0, 1.0, 0.16), 0.0)?;
                    }
                    for (slot, &i) in order.iter().enumerate() {
                        let (name, time, now) = &f.times[i];
                        let x = row_x + slot as f32 * colw;
                        let mut a = self.piece(name, &lf, colw, ta, 0.0, if *now { acc } else { faint })?;
                        a.x = x;
                        a.y = row_y + 0.9 * u;
                        draw(&a)?;
                        let mut b = self.piece(time, &vf, colw, ta, 0.0, if *now { white } else { soft })?;
                        b.x = x;
                        b.y = row_y + 2.0 * u;
                        draw(&b)?;
                    }
                }
            }
        }

        // ---- system strip ----
        if let Some((s, hist, labels)) = &f.monitor {
            let (sx, sw) = (col_l, cw);
            let cell = sw / 5.0;
            let lf = self.format('e', 0.6 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, false)?;
            let vf = self.format('e', 0.84 * u, DWRITE_FONT_WEIGHT_NORMAL, false)?;
            let values = [
                format!("{:.0}%", s.cpu * 100.0),
                format!("{:.1} / {:.0} GB", s.mem_used_gb, s.mem_total_gb),
                format!("{:.0}%", s.gpu * 100.0),
                format!("{:.0}%", s.disk * 100.0),
                format!("↓ {}", crate::sysmon::rate(s.down)),
            ];
            let level = [s.cpu, s.mem, s.gpu, s.disk, hist[4].last().copied().unwrap_or(0.0)].map(|v| v.clamp(0.0, 1.0));
            let line = unsafe { t.CreateSolidColorBrush(&rgba(acc.r, acc.g, acc.b, 0.85), None)? };
            let track = unsafe { t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.15), None)? };
            match st.monitor_style.as_str() {
                "text" => {
                    // one quiet line: CPU 8%   MEMORY 10.4 / 28 GB   …
                    let mut x = sx;
                    for i in 0..5 {
                        let mut a = self.piece(&labels[i], &lf, 20.0 * u, 0, 0.08 * u, faint)?;
                        let lw = self.width(&a)?;
                        a.x = x;
                        a.y = strip_y + 0.2 * u;
                        draw(&a)?;
                        let mut b = self.piece(&values[i], &vf, 20.0 * u, 0, 0.0, soft)?;
                        let vw = self.width(&b)?;
                        b.x = x + lw + 0.4 * u;
                        b.y = strip_y;
                        draw(&b)?;
                        x += lw + vw + 1.6 * u;
                        if x > sx + sw {
                            break;
                        }
                    }
                }
                "rings" => {
                    // round gauges with the percentage inside
                    let rad = (cell * 0.5 - 0.8 * u).min(2.0 * u);
                    for i in 0..5 {
                        let cx = sx + i as f32 * cell + cell / 2.0;
                        let cyy = strip_y + rad + 0.2 * u;
                        unsafe {
                            let e = D2D1_ELLIPSE { point: Vector2 { X: cx, Y: cyy }, radiusX: rad, radiusY: rad };
                            t.DrawEllipse(&e, &track, 0.32 * u, None);
                            let v = level[i].max(0.005).min(0.999);
                            let a0 = -std::f32::consts::FRAC_PI_2;
                            let a1 = a0 + v * std::f32::consts::TAU;
                            let geo = self.factory.CreatePathGeometry()?;
                            let sink = geo.Open()?;
                            sink.BeginFigure(Vector2 { X: cx + rad * a0.cos(), Y: cyy + rad * a0.sin() }, D2D1_FIGURE_BEGIN_HOLLOW);
                            sink.AddArc(&D2D1_ARC_SEGMENT {
                                point: Vector2 { X: cx + rad * a1.cos(), Y: cyy + rad * a1.sin() },
                                size: D2D_SIZE_F { width: rad, height: rad },
                                rotationAngle: 0.0,
                                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                                arcSize: if v > 0.5 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
                            });
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                            sink.Close()?;
                            t.DrawGeometry(&geo, &line, 0.32 * u, None);
                        }
                        let pct = if i == 1 { format!("{:.0}%", s.mem * 100.0) } else if i == 4 { crate::sysmon::rate(s.down).replace(" ", "\n") } else { values[i].clone() };
                        let mut b = self.piece(&pct, &self.format('e', 0.72 * u, DWRITE_FONT_WEIGHT_NORMAL, false)?, rad * 2.0, 1, 0.0, soft)?;
                        b.x = cx - rad;
                        b.y = cyy - b.h / 2.0;
                        draw(&b)?;
                        let mut a = self.piece(&labels[i], &lf, cell, 1, 0.08 * u, faint)?;
                        a.x = cx - cell / 2.0;
                        a.y = cyy + rad + 0.5 * u;
                        draw(&a)?;
                    }
                }
                style => {
                    for i in 0..5 {
                        let x = sx + i as f32 * cell;
                        let mut a = self.piece(&labels[i], &lf, cell, 0, 0.08 * u, faint)?;
                        a.x = x;
                        a.y = strip_y;
                        draw(&a)?;
                        let mut b = self.piece(&values[i], &vf, cell - 0.4 * u, 0, 0.0, soft)?;
                        b.x = x;
                        b.y = strip_y + 0.95 * u;
                        draw(&b)?;
                        if style == "bars" {
                            // a fill bar for how busy it is
                            let (bx, by, bw) = (x, strip_y + 2.35 * u, cell - 1.4 * u);
                            unsafe {
                                let q = |r: D2D_RECT_F| D2D1_ROUNDED_RECT { rect: r, radiusX: 0.15 * u, radiusY: 0.15 * u };
                                t.FillRoundedRectangle(&q(rect(bx, by, bx + bw, by + 0.32 * u)), &track);
                                t.FillRoundedRectangle(&q(rect(bx, by, bx + bw * level[i].max(0.03), by + 0.32 * u)), &line);
                            }
                            continue;
                        }
                        let pts = &hist[i];
                        if pts.len() > 1 {
                            unsafe {
                                let geo = self.factory.CreatePathGeometry()?;
                                let sink = geo.Open()?;
                                let (gx, gy, gw, gh) = (x, strip_y + 2.45 * u, cell - 1.4 * u, 0.8 * u);
                                let step = gw / (crate::sysmon::HISTORY - 1) as f32;
                                let off = (crate::sysmon::HISTORY - pts.len()) as f32 * step;
                                sink.BeginFigure(Vector2 { X: gx + off, Y: gy + gh * (1.0 - pts[0]) }, D2D1_FIGURE_BEGIN_HOLLOW);
                                let rest: Vec<Vector2> = pts.iter().enumerate().skip(1).map(|(k, v)| Vector2 { X: gx + off + k as f32 * step, Y: gy + gh * (1.0 - v) }).collect();
                                sink.AddLines(&rest);
                                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                                sink.Close()?;
                                t.DrawGeometry(&geo, &line, 1.2, None);
                            }
                        }
                    }
                }
            }
            self.strip_rect.borrow_mut().insert(mi, RECT { left: sx as i32 - 4, top: strip_y as i32 - 4, right: (sx + sw) as i32 + 4, bottom: (strip_y + strip_h) as i32 + 8 });
        }
        Ok(())
    }

    /// Draws one `w`×`h` frame to a PNG (screenshots and checks), without touching the desktop.
    pub fn snapshot(&self, path: &str, w: u32, h: u32, f: &Frame, st: &Style) -> Result<()> {
        unsafe {
            let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let bmp = wic.CreateBitmap(w, h, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let t = self.factory.CreateWicBitmapRenderTarget(&bmp, &props)?;
            let m = RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 };
            t.BeginDraw();
            self.picture(&t, 0, m, f, &Matrix3x2::identity())?;
            self.panel(&t, 0, m, f, st)?;
            t.EndDraw(None, None)?;
            let stream = wic.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path), GENERIC_WRITE.0)?;
            let enc = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame = None;
            enc.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
            let frame = frame.unwrap();
            frame.Initialize(None)?;
            frame.WriteSource(&bmp, std::ptr::null())?;
            frame.Commit()?;
            enc.Commit()?;
        }
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.dwrite.UnregisterFontFileLoader(&self.loader);
        }
    }
}

/// Decodes `src`, scales it to cover `w`×`h` (cropping the middle) and applies the picture
/// effect. Also returns, for the glass card, a soft blurred copy at a third of the size.
/// `glass_blur` 0..30: how soft the glass card's background is (0 = clear glass).
#[allow(clippy::too_many_arguments)]
pub(crate) fn load_photo(src: &Photo, w: u32, h: u32, effect: &str, grain: bool, glass: bool, glass_blur: u32) -> Result<(IWICBitmap, Option<IWICBitmap>)> {
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let mut photo = match src {
            Photo::Calligraphy(i, flip, en) => crate::calligraphy::render(*i, w, h, *flip, en)?,
            _ => {
                let dec = match src {
                    Photo::Embedded(bytes) => {
                        let stream = wic.CreateStream()?;
                        stream.InitializeFromMemory(bytes)?;
                        wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?
                    }
                    Photo::File(path) => wic.CreateDecoderFromFilename(&HSTRING::from(path.as_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?,
                    Photo::Calligraphy(..) => unreachable!(),
                };
                let frame = dec.GetFrame(0)?;
                let (mut iw, mut ih) = (0u32, 0u32);
                frame.GetSize(&mut iw, &mut ih)?;
                let s = (w as f64 / iw as f64).max(h as f64 / ih as f64);
                let (sw, sh) = ((((iw as f64) * s).ceil() as u32).max(w), (((ih as f64) * s).ceil() as u32).max(h));
                let scaler = wic.CreateBitmapScaler()?;
                scaler.Initialize(&frame, sw, sh, WICBitmapInterpolationModeHighQualityCubic)?;
                let clip = wic.CreateBitmapClipper()?;
                let r = WICRect { X: ((sw - w) / 2) as i32, Y: ((sh - h) / 2) as i32, Width: w as i32, Height: h as i32 };
                clip.Initialize(&scaler, &r)?;
                let conv = wic.CreateFormatConverter()?;
                conv.Initialize(&clip, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeMedianCut)?;
                wic.CreateBitmapFromSource(&conv, WICBitmapCacheOnLoad)?
            }
        };

        // Blur by shrinking and growing back: cheap, and smooth with a good scaler.
        let blur = |src: &IWICBitmap, div: u32, back_w: u32, back_h: u32| -> Result<IWICBitmap> {
            let small = wic.CreateBitmapScaler()?;
            small.Initialize(src, (w / div).max(1), (h / div).max(1), WICBitmapInterpolationModeFant)?;
            let big = wic.CreateBitmapScaler()?;
            big.Initialize(&small, back_w.max(1), back_h.max(1), WICBitmapInterpolationModeHighQualityCubic)?;
            wic.CreateBitmapFromSource(&big, WICBitmapCacheOnLoad)
        };
        if effect == "blur" {
            photo = blur(&photo, 10, w, h)?;
        }
        if matches!(effect, "warm" | "cool" | "mono") || grain {
            let lock = photo.Lock(&WICRect { X: 0, Y: 0, Width: w as i32, Height: h as i32 }, WICBitmapLockWrite.0 as u32)?;
            let stride = lock.GetStride()? as usize;
            let (mut size, mut ptr) = (0u32, std::ptr::null_mut());
            lock.GetDataPointer(&mut size, &mut ptr)?;
            let px = std::slice::from_raw_parts_mut(ptr, size as usize);
            let mut seed: u32 = 0x9E37_79B9;
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let i = y * stride + x * 4;
                    let (mut b, mut g, mut r) = (px[i] as f32, px[i + 1] as f32, px[i + 2] as f32);
                    match effect {
                        "warm" => {
                            r = r * 1.06 + 8.0;
                            g *= 1.01;
                            b *= 0.88;
                        }
                        "cool" => {
                            r *= 0.9;
                            g *= 0.99;
                            b = b * 1.06 + 8.0;
                        }
                        "mono" => {
                            let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                            (r, g, b) = (l, l, l);
                        }
                        _ => {}
                    }
                    if grain {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let n = (seed & 0xFF) as f32 / 255.0 * 14.0 - 7.0;
                        r += n;
                        g += n;
                        b += n;
                    }
                    px[i] = b.clamp(0.0, 255.0) as u8;
                    px[i + 1] = g.clamp(0.0, 255.0) as u8;
                    px[i + 2] = r.clamp(0.0, 255.0) as u8;
                }
            }
        }
        let soft = if glass && glass_blur > 0 { Some(blur(&photo, 4 + glass_blur.min(30), w / 3, h / 3)?) } else { None };
        Ok((photo, soft))
    }
}
