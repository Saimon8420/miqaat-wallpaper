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
    serif: (IDWriteFontCollection1, HSTRING),
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

impl Renderer {
    /// `photos`: one source per monitor with that monitor's size.
    pub fn new(photos: &[(Photo, u32, u32)], effect: &str, grain: bool, glass: bool, font_ar: &str, font_bn: &str, font_en: &str) -> Result<Self> {
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
                serif: load(pick(crate::assets::ENGLISH_FONTS, "Playfair Display"))?,
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
                let (photo, soft) = load_photo(src, *w, *h, effect, grain, glass)?;
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
            's' => &self.fonts.serif,
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
        let white = rgba(0.97, 0.95, 0.92, 1.0);
        let soft = rgba(0.97, 0.95, 0.92, 0.76);
        let faint = rgba(0.97, 0.95, 0.92, 0.55);
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
                let (l, r) = if st.side_left { (x0 + 3.2 * u, x0 + 34.2 * u) } else { (x0 + w - 34.2 * u, x0 + w - 3.2 * u) };
                (l, r, 0, y0 + 3.2 * u, y0 + h - 3.2 * u, 8.4 * u)
            }
        };
        let cw = col_r - col_l;

        // In the bar the screen is split in three: clock | reminder | prayer row.
        let (clock_x, clock_w) = if bar { (col_l, cw * 0.25) } else { (col_l, cw) };
        let (rem_x, rem_w) = if bar { (col_l + cw * 0.28, cw * 0.4) } else { (col_l, cw) };
        let (row_x, row_w) = if bar { (col_l + cw * 0.71, cw * 0.29) } else { (col_l, cw) };

        // ---- clock group ----
        let (clock_font, clock_weight) = match st.clock_style.as_str() {
            "bold" => ('e', DWRITE_FONT_WEIGHT_SEMI_BOLD),
            "serif" => ('s', DWRITE_FONT_WEIGHT_NORMAL),
            _ => ('e', DWRITE_FONT_WEIGHT_EXTRA_LIGHT),
        };
        let mut clock_text = f.clock.clone();
        if let Some(s) = &f.seconds {
            clock_text.push_str(s);
        }
        let has_next = st.show_next && !f.next.is_empty();
        let mut tops = vec![
            (self.piece(&f.meta.to_uppercase(), &self.format(label, 0.95 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, st.rtl_labels)?, clock_w, align, 0.12 * u * track, faint)?, 0.2 * u),
            (self.piece(&clock_text, &self.format(clock_font, clock_size, clock_weight, false)?, clock_w, align, 0.0, white)?, -0.4 * u),
        ];
        if has_next {
            let size = if corner || bar { 1.15 * u } else { 1.35 * u };
            tops.push((self.piece(&f.next, &self.format(label, size, DWRITE_FONT_WEIGHT_LIGHT, st.rtl_labels)?, clock_w, align, 0.0, white)?, 0.0));
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
        let gaps = 3.0 * u + if row_h > 0.0 { 1.6 * u } else { 0.0 } + if strip_h > 0.0 { 1.4 * u } else { 0.0 };
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
                rems.push((self.piece(&f.kind.to_uppercase(), &self.format(label, 0.8 * u, DWRITE_FONT_WEIGHT_SEMI_BOLD, st.rtl_labels)?, rem_w, align, 0.1 * u * track, acc)?, 0.45 * u));
                if !f.arabic.is_empty() {
                    let max = if bar { 5.2 * u } else { 7.5 * u } * shrink;
                    rems.push((self.fitted(&f.arabic, 'a', 2.3 * u * st.ar_scale, DWRITE_FONT_WEIGHT_NORMAL, true, rem_w, max, align, white)?, 0.45 * u));
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
        let mut cy = if corner {
            rem_top - 2.0 * u - span
        } else if bar {
            rem_bottom - span
        } else {
            top
        };
        // Together: close the gap between the clock and the rest, and centre the whole block.
        if st.together && !corner && !bar {
            let gap = 3.0 * u;
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
        for (mut p, gap) in tops {
            p.x = clock_x;
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
                        let rr = D2D1_ROUNDED_RECT { rect: b, radiusX: 1.4 * u, radiusY: 1.4 * u };
                        if let Some(bmp) = &soft_bmp {
                            let brush = t.CreateBitmapBrush(bmp, None, None)?;
                            let sz = bmp.GetPixelSize();
                            brush.SetTransform(&(Matrix3x2::scale(w / sz.width.max(1) as f32, h / sz.height.max(1) as f32) * Matrix3x2::translation(x0, y0)));
                            t.FillRoundedRectangle(&rr, &brush);
                        }
                        t.FillRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(0.03, 0.04, 0.07, 0.2 + 0.45 * a), None)?);
                        t.DrawRoundedRectangle(&rr, &t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.14), None)?, 1.0, None);
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
        let shadow = st.behind == "none";
        let draw = |p: &Piece| -> Result<()> {
            unsafe {
                if shadow {
                    let s = t.CreateSolidColorBrush(&rgba(0.0, 0.0, 0.0, 0.6 * p.color.a), None)?;
                    t.DrawTextLayout(Vector2 { X: p.x + 0.1 * u, Y: p.y + 0.12 * u }, &p.layout, &s, D2D1_DRAW_TEXT_OPTIONS_NONE);
                }
                t.DrawTextLayout(Vector2 { X: p.x, Y: p.y }, &p.layout, &t.CreateSolidColorBrush(&p.color, None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
            }
            Ok(())
        };
        for p in &clock_pieces {
            draw(p)?;
        }
        unsafe {
            // am/pm sits right after the clock digits; remember the clock's box for seconds.
            let c = &clock_pieces[1];
            let mut cm = DWRITE_TEXT_METRICS::default();
            c.layout.GetMetrics(&mut cm)?;
            let (dl, dr) = (c.x + cm.left, c.x + cm.left + cm.width);
            if !f.ampm.is_empty() {
                let mut ap = ampm;
                ap.x = dr + 0.6 * u;
                ap.y = c.y + c.h * 0.66 - ap.h;
                draw(&ap)?;
            }
            self.clock_rect.borrow_mut().insert(mi, RECT { left: (dl - u) as i32, top: c.y as i32, right: (dr + 7.0 * u) as i32, bottom: (c.y + c.h) as i32 });
        }
        if has_next {
            // progress from the last prayer to the next one
            let n = clock_pieces.last().unwrap();
            let ly = n.y + n.h + 0.55 * u;
            let len = if centre { cw * 0.3 } else { clock_w.min(16.0 * u) };
            let lx = match align {
                1 => clock_x + (clock_w - len) / 2.0,
                2 => clock_x + clock_w - len,
                _ => clock_x,
            };
            unsafe {
                t.FillRectangle(&rect(lx, ly, lx + len, ly + 0.14 * u), &t.CreateSolidColorBrush(&rgba(1.0, 1.0, 1.0, 0.18), None)?);
                t.FillRectangle(&rect(lx, ly, lx + len * f.progress.clamp(0.0, 1.0), ly + 0.14 * u), &t.CreateSolidColorBrush(&acc, None)?);
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
                        if k + 1 < n {
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
                    fill(rect(row_x, row_y, row_x + row_w, row_y + 1.0), rgba(1.0, 1.0, 1.0, 0.16), 0.0)?;
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
pub(crate) fn load_photo(src: &Photo, w: u32, h: u32, effect: &str, grain: bool, glass: bool) -> Result<(IWICBitmap, Option<IWICBitmap>)> {
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let dec = match src {
            Photo::Embedded(bytes) => {
                let stream = wic.CreateStream()?;
                stream.InitializeFromMemory(bytes)?;
                wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?
            }
            Photo::File(path) => wic.CreateDecoderFromFilename(&HSTRING::from(path.as_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?,
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
        let mut photo = wic.CreateBitmapFromSource(&conv, WICBitmapCacheOnLoad)?;

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
        let soft = if glass { Some(blur(&photo, 14, w / 3, h / 3)?) } else { None };
        Ok((photo, soft))
    }
}
