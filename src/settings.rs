//! User settings, saved as JSON in %APPDATA%\Miqaat Wallpaper\settings.json.
//!
//! Choices are stored as short strings ("right", "glass", …) so the file stays readable and
//! older files keep loading when new options are added (missing fields take the default).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Location & times
    pub place: String,
    pub lat: f64,
    pub lng: f64,
    pub method: String,
    pub madhab: String,
    pub hijri_adjust: i32,
    pub clock24: bool,
    pub notify: bool,
    // Reminders
    pub kinds: Vec<String>,
    pub interval: u32,
    pub follow_time: bool,
    pub special_days: bool,
    pub meaning: String,
    // Appearance
    pub scene: String,
    pub custom_photo: String,
    pub rotate: String,
    pub shade: u32,
    pub side: String,
    pub show_next: bool,
    pub show_reminder: bool,
    pub show_special: bool,
    pub show_times: bool,
    pub show_monitor: bool,
    pub light_follows: bool,
    pub night_dark: u32,
    // Design
    pub layout: String,
    /// "together": clock, reminder and times as one centred block; "spread": clock at the top,
    /// the rest at the bottom.
    pub spacing: String,
    /// Prayer times as a "row", "grid" (cards), "list" or "timeline".
    pub times_style: String,
    /// System monitor as "lines", "bars", "rings" or "text".
    pub monitor_style: String,
    pub clock_style: String,
    pub arabic_digits: bool,
    pub seconds: bool,
    /// A preset name (gold, emerald, sky, rose, pearl) or a custom "#RRGGBB".
    pub accent: String,
    /// Custom colours picked recently, newest first.
    pub recent_colors: Vec<String>,
    pub text_size: u32,
    pub behind: String,
    pub drift: bool,
    pub grain: bool,
    pub effect: String,
    // Fonts & language
    pub lang: String,
    pub font_ar: String,
    pub ar_size: u32,
    pub font_bn: String,
    pub font_en: String,
    // System
    pub autostart: bool,
    pub pause_fullscreen: bool,
    pub battery_saver: bool,
    pub monitor_every: u32,
    pub monitors: String,
    /// Where the clock panel goes when several screens are connected: "main", "all", or screen
    /// numbers counted left to right, e.g. "1,3".
    pub panel_on: String,
    // 1.1.3: design
    /// "" = same as the English font.
    pub clock_font: String,
    pub clock_size: u32,
    /// "in", "at" or "timer".
    pub countdown: String,
    /// "line", "dots", "ring" or "off".
    pub progress_style: String,
    pub show_weekday: bool,
    pub show_hijri: bool,
    pub show_date: bool,
    pub show_city: bool,
    pub caps: bool,
    /// "white", "warm" or "cool".
    pub text_tone: String,
    /// "off", "soft" or "strong".
    pub text_shadow: String,
    /// "layout" or "right".
    pub ar_align: String,
    pub col_width: u32,
    /// "dark" or "light".
    pub glass_tint: String,
    pub glass_blur: u32,
    /// "square", "round" or "very".
    pub corners: String,
    pub show_sunrise: bool,
    /// "current" or "next".
    pub highlight: String,
    pub dividers: bool,
    // 1.1.3: features
    pub show_fasting: bool,
    pub show_qibla: bool,
    pub show_event: bool,
    /// Minutes before each prayer for a reminder notification (0 = off).
    pub notify_before: u32,
    pub jumuah: bool,
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            place: "Dhaka, Bangladesh".into(),
            lat: 23.8103,
            lng: 90.4125,
            method: "Karachi".into(),
            madhab: "hanafi".into(),
            hijri_adjust: 0,
            clock24: false,
            notify: true,
            kinds: ["quran", "hadith", "dua", "dhikr", "istighfar", "durood"].map(String::from).to_vec(),
            interval: 15,
            follow_time: true,
            special_days: true,
            meaning: "en".into(),
            scene: "kaaba".into(),
            custom_photo: String::new(),
            rotate: "daily".into(),
            shade: 70,
            side: "right".into(),
            show_next: true,
            show_reminder: true,
            show_special: true,
            show_times: true,
            show_monitor: true,
            light_follows: true,
            night_dark: 55,
            layout: "column".into(),
            spacing: "together".into(),
            times_style: "row".into(),
            monitor_style: "lines".into(),
            clock_style: "thin".into(),
            arabic_digits: false,
            seconds: false,
            accent: "gold".into(),
            recent_colors: vec![],
            text_size: 100,
            behind: "shade".into(),
            drift: false,
            grain: false,
            effect: "none".into(),
            lang: "en".into(),
            font_ar: "Noto Naskh Arabic".into(),
            ar_size: 100,
            font_bn: "Noto Sans Bengali".into(),
            font_en: "Geist".into(),
            autostart: true,
            pause_fullscreen: true,
            battery_saver: true,
            monitor_every: 2,
            monitors: "same".into(),
            panel_on: "main".into(),
            clock_font: String::new(),
            clock_size: 100,
            countdown: "in".into(),
            progress_style: "line".into(),
            show_weekday: true,
            show_hijri: true,
            show_date: false,
            show_city: true,
            caps: true,
            text_tone: "white".into(),
            text_shadow: "off".into(),
            ar_align: "layout".into(),
            col_width: 31,
            glass_tint: "dark".into(),
            glass_blur: 10,
            corners: "round".into(),
            show_sunrise: true,
            highlight: "current".into(),
            dividers: true,
            show_fasting: true,
            show_qibla: true,
            show_event: true,
            notify_before: 10,
            jumuah: true,
            auto_update: true,
        }
    }
}

/// %APPDATA%\Miqaat Wallpaper (settings and the saved month of prayer times).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Miqaat Wallpaper")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(data_dir().join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), s);
        }
    }

    pub fn has(&self, kind: &str) -> bool {
        self.kinds.iter().any(|k| k == kind)
    }

    /// Fields that change which prayer times are fetched.
    pub fn times_key(&self) -> (String, String, String) {
        (format!("{:.4},{:.4}", self.lat, self.lng), self.method.clone(), self.madhab.clone())
    }

    /// Fields that change the decoded pictures or fonts (anything else is just a repaint).
    pub fn look_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.scene, self.custom_photo, self.rotate, self.effect, self.grain, self.monitors, self.font_ar, self.font_bn, self.font_en, self.behind,
            self.clock_font, self.clock_style, self.glass_blur
        )
    }
}
