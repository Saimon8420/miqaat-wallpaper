//! Calligraphy scenes: a vector background (SVG, drawn by resvg) with Kufi lettering on it
//! (drawn by DirectWrite with the fonts built into the app).
//!
//! A scene is drawn once, at the monitor's size, whenever the picture changes, and from then on
//! it is just a bitmap like a photo. Nothing is redrawn while it is on screen.

use crate::assets::{Calligraphy, CALLIGRAPHY};
use crate::render::{load_font, parse_hex};
use windows::core::{w, Interface, Result, HSTRING};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows_numerics::Vector2;

/// The scenes picked by hand (not the Ramadan and Eid ones).
pub fn everyday() -> Vec<usize> {
    (0..CALLIGRAPHY.len()).filter(|&i| !CALLIGRAPHY[i].occasion).collect()
}

pub fn index(id: &str) -> Option<usize> {
    CALLIGRAPHY.iter().position(|c| c.id == id)
}

/// The scene for a special day, if today is one (Hijri month and day).
pub fn occasion(month: u32, day: u32) -> Option<usize> {
    let id = match (month, day) {
        (9, 1..=20) => ["ramadan-night", "month-of-the-qur-an", "lantern-street"][(day as usize - 1) % 3],
        (9, 21..=30) => "laylat-al-qadr",
        (10, 1..=3) => ["eid-morning", "eid-card", "takbir-of-eid"][day as usize - 1],
        (12, 9) => "day-of-arafah",
        (12, 10..=13) => ["eid-prayer-at-the-eidgah", "takbir-of-eid"][(day as usize - 10) % 2],
        _ => return None,
    };
    index(id)
}

/// True when the scene is light, so the clock panel needs dark text and no dark shade.
pub fn light(c: &Calligraphy) -> bool {
    let [r, g, b] = parse_hex(c.panel).unwrap_or([1.0; 3]);
    0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5
}

/// Some lettering is drawn larger or smaller so every font looks about the same size.
fn font_factor(name: &str) -> f32 {
    match name {
        "Noto Kufi Arabic" => 0.9,
        "Kufam" => 0.92,
        "Changa" => 0.95,
        "Alexandria" => 0.86,
        "Lalezar" => 1.08,
        "Readex Pro" => 0.88,
        _ => 1.0,
    }
}

fn color(hex: &str, a: f32) -> D2D1_COLOR_F {
    let [r, g, b] = parse_hex(hex).unwrap_or([1.0; 3]);
    D2D1_COLOR_F { r, g, b, a }
}

/// Draws scene `idx` at `w`×`h`. `flip` mirrors the picture (not the writing) when the clock
/// panel is on the left, so the lettering stays on the free side.
pub fn render(idx: usize, w: u32, h: u32, flip: bool, en_font: &str) -> Result<IWICBitmap> {
    let c = &CALLIGRAPHY[idx];
    let (wf, hf) = (w as f32, h as f32);
    let s = (wf / 1600.0).max(hf / 900.0);
    let (ox, oy) = ((wf - 1600.0 * s) / 2.0, (hf - 900.0 * s) / 2.0);

    // ---- the background ----
    let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_OUTOFMEMORY))?;
    if let Ok(tree) = resvg::usvg::Tree::from_str(c.svg, &resvg::usvg::Options::default()) {
        let t = if flip { resvg::tiny_skia::Transform::from_row(-s, 0.0, 0.0, s, ox + 1600.0 * s, oy) } else { resvg::tiny_skia::Transform::from_row(s, 0.0, 0.0, s, ox, oy) };
        resvg::render(&tree, t, &mut pm.as_mut());
    }
    // RGBA to BGRA, plus the paper grain the scene asks for (cheaper than the SVG filter).
    let mut seed: u32 = 0x9E37_79B9;
    let amp = c.grain * 40.0;
    for px in pm.data_mut().chunks_exact_mut(4) {
        px.swap(0, 2);
        if amp > 0.0 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let n = ((seed & 0xFF) as f32 / 255.0 * 2.0 - 1.0) * amp;
            for v in &mut px[..3] {
                *v = (*v as f32 + n).clamp(0.0, 255.0) as u8;
            }
        }
    }

    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bmp = wic.CreateBitmapFromMemory(w, h, &GUID_WICPixelFormat32bppPBGRA, w * 4, pm.data())?;
        drop(pm);

        // ---- the lettering ----
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let t = factory.CreateWicBitmapRenderTarget(&bmp, &props)?;
        let dwrite: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
        let loader = dwrite.CreateInMemoryFontFileLoader()?;
        dwrite.RegisterFontFileLoader(&loader)?;
        let result = (|| -> Result<()> {
            let pick = |list: &'static [crate::assets::Font], name: &str| list.iter().find(|f| f.name == name).unwrap_or(&list[0]);
            let ar = load_font(&dwrite, &loader, pick(crate::assets::ARABIC_FONTS, c.font))?;
            let en = load_font(&dwrite, &loader, pick(crate::assets::ENGLISH_FONTS, en_font))?;
            let format = |f: &(IDWriteFontCollection1, HSTRING), size: f32, rtl: bool| -> Result<IDWriteTextFormat> {
                let fmt = dwrite.CreateTextFormat(&f.1, &f.0.cast::<IDWriteFontCollection>()?, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size.max(1.0), w!("en-us"))?;
                fmt.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                if rtl {
                    fmt.SetReadingDirection(DWRITE_READING_DIRECTION_RIGHT_TO_LEFT)?;
                }
                Ok(fmt)
            };
            let layout = |text: &str, fmt: &IDWriteTextFormat, width: f32| -> Result<(IDWriteTextLayout, f32, f32)> {
                let wide: Vec<u16> = text.encode_utf16().collect();
                let l = dwrite.CreateTextLayout(&wide, fmt, width.max(1.0), 100_000.0)?;
                let mut m = DWRITE_TEXT_METRICS::default();
                l.GetMetrics(&mut m)?;
                Ok((l, m.width, m.height))
            };
            let mx = |x: f32| ox + if flip { 1600.0 - x } else { x } * s;
            t.BeginDraw();

            // a large faint word behind everything, in some scenes
            if let Some(wm) = &c.watermark {
                let fmt = format(&pick_font(&dwrite, &loader, wm.font)?, wm.size * s, true)?;
                fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let (l, lw, _) = layout(wm.text, &fmt, 4000.0)?;
                let mut lm = [DWRITE_LINE_METRICS::default(); 1];
                let mut n = 0;
                let _ = l.GetLineMetrics(Some(&mut lm), &mut n);
                let x = mx(wm.x) - 2000.0;
                let _ = lw;
                t.DrawTextLayout(Vector2 { X: x, Y: oy + wm.y * s - lm[0].baseline }, &l, &t.CreateSolidColorBrush(&color(wm.color, 1.0), None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
            }

            // the box, in pixels
            let [x1, y1, x2, y2] = c.bx;
            let (bl, br) = if flip { (mx(x2), mx(x1)) } else { (mx(x1), mx(x2)) };
            let (bt, bb) = (oy + y1 * s, oy + y2 * s);
            let (bw, bh) = (br - bl, bb - bt);
            let n = c.lines.len() as f32;
            let longest = c.lines.iter().map(|l| l.chars().count()).max().unwrap_or(5).max(5) as f32;
            let meaning = !c.no_meaning;
            // the same starting size as the design, then shrink until it fits the box
            let fit = (x2 - x1) * 1.75 / longest;
            let m = if meaning { 1.2 } else { 0.0 };
            let mut size = (fit * 1.05).min((y2 - y1) / (n * 1.38 + m) * 0.95) * c.scale * font_factor(c.font);
            size = size.min(fit * 1.25) * s;

            let en_fmt = format(&en, 16.8 * s, false)?;
            let ref_fmt = format(&en, 12.5 * s, false)?;
            let mean = if meaning { Some(layout(c.meaning, &en_fmt, (448.0 * s).min(bw))?) } else { None };
            let refl = if meaning {
                let up = c.reference.to_uppercase();
                let (l, _, _) = layout(&up, &ref_fmt, bw)?;
                let l1: IDWriteTextLayout1 = l.cast()?;
                l1.SetCharacterSpacing(0.0, 0.2 * 12.5 * s, 0.0, DWRITE_TEXT_RANGE { startPosition: 0, length: up.encode_utf16().count() as u32 })?;
                let mut mt = DWRITE_TEXT_METRICS::default();
                l.GetMetrics(&mut mt)?;
                Some((l, mt.height))
            } else {
                None
            };
            let below = match (&mean, &refl) {
                (Some(m), Some(r)) => 20.8 * s + m.2 + 6.4 * s + r.1,
                _ => 0.0,
            };
            let ar_text = c.lines.join("\n");
            let (mut arl, mut aw, mut ah);
            loop {
                let fmt = format(&ar, size, true)?;
                fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                fmt.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, size * 1.38, size * 1.1)?;
                let got = layout(&ar_text, &fmt, bw)?;
                (arl, aw, ah) = (got.0, got.1, got.2);
                if (aw <= bw * 0.94 && ah + below <= bh + 2.0) || size < 8.0 {
                    break;
                }
                size *= 0.94;
            }
            let _ = aw;
            let ink = c.ink;
            let mut y = bt + (bh - ah - below) / 2.0;
            t.DrawTextLayout(Vector2 { X: bl, Y: y }, &arl, &t.CreateSolidColorBrush(&color(ink, 1.0), None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
            y += ah;
            if let (Some(m), Some(r)) = (&mean, &refl) {
                y += 20.8 * s;
                let mw = (448.0 * s).min(bw);
                t.DrawTextLayout(Vector2 { X: bl + (bw - mw) / 2.0, Y: y }, &m.0, &t.CreateSolidColorBrush(&color(ink, 0.8), None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
                y += m.2 + 6.4 * s;
                t.DrawTextLayout(Vector2 { X: bl, Y: y }, &r.0, &t.CreateSolidColorBrush(&color(ink, 0.6), None)?, D2D1_DRAW_TEXT_OPTIONS_NONE);
            }
            t.EndDraw(None, None)?;
            Ok(())
        })();
        let _ = dwrite.UnregisterFontFileLoader(&loader);
        result?;
        Ok(bmp)
    }
}

fn pick_font(dwrite: &IDWriteFactory5, loader: &IDWriteInMemoryFontFileLoader, name: &str) -> Result<(IDWriteFontCollection1, HSTRING)> {
    let list = crate::assets::ARABIC_FONTS;
    load_font(dwrite, loader, list.iter().find(|f| f.name == name).unwrap_or(&list[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occasions_by_hijri_date() {
        let id = |m, d| occasion(m, d).map(|i| CALLIGRAPHY[i].id);
        assert_eq!(id(9, 1), Some("ramadan-night"));
        assert_eq!(id(9, 27), Some("laylat-al-qadr"));
        assert_eq!(id(10, 1), Some("eid-morning"));
        assert_eq!(id(12, 9), Some("day-of-arafah"));
        assert_eq!(id(12, 10), Some("eid-prayer-at-the-eidgah"));
        assert_eq!(id(12, 14), None);
        assert_eq!(id(5, 24), None);
        assert_eq!(everyday().len(), 21);
    }
}
