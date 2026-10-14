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
            show_monitor: false,
            light_follows: true,
            night_dark: 55,
            layout: "column".into(),
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
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.scene, self.custom_photo, self.rotate, self.effect, self.grain, self.monitors, self.font_ar, self.font_bn, self.font_en, self.behind
        )
    }
}
