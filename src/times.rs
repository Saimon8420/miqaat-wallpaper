//! Prayer times from the Miqaat API, one month at a time, saved to disk.
//!
//! A month is fetched once and kept in %APPDATA%\Miqaat Wallpaper\times-*.json, so the
//! wallpaper makes about one request a month and keeps working offline. Times are kept in the
//! chosen city's own clock (the API's ISO offset), so a city in another time zone still works.

use serde::{Deserialize, Serialize};

pub const API: &str = "https://miqaat-dev.vercel.app/v1";

#[derive(Clone, Serialize, Deserialize)]
pub struct Day {
    /// "2026-10-14"
    pub date: String,
    /// "3 Jumada al-Awwal 1448"
    pub hijri: String,
    /// Fajr, Sunrise, Dhuhr, Asr, Maghrib, Isha as minutes after the city's midnight.
    pub minutes: [u32; 6],
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Month {
    pub key: String,
    /// City's offset from UTC in minutes (from the API's ISO times).
    pub offset: i32,
    pub days: Vec<Day>,
}

#[derive(Deserialize)]
struct Resp {
    data: RespData,
}
#[derive(Deserialize)]
struct RespData {
    days: Vec<RespDay>,
}
#[derive(Deserialize)]
struct RespDay {
    date: String,
    hijri: String,
    prayers: Prayers,
}
#[derive(Deserialize)]
struct Prayers {
    fajr: Time,
    sunrise: Time,
    dhuhr: Time,
    asr: Time,
    maghrib: Time,
    isha: Time,
}
#[derive(Deserialize)]
struct Time {
    iso: String,
}

/// "2026-10-14T04:40:00+06:00" -> 280 (minutes after midnight, local to the city).
fn minutes(iso: &str) -> u32 {
    let h: u32 = iso.get(11..13).and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u32 = iso.get(14..16).and_then(|s| s.parse().ok()).unwrap_or(0);
    h * 60 + m
}

/// "...+06:00" -> 360, "...-04:30" -> -270, "...Z" -> 0.
fn offset(iso: &str) -> i32 {
    let tail = &iso[iso.len().saturating_sub(6)..];
    let sign = match tail.as_bytes().first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return 0,
    };
    let h: i32 = tail.get(1..3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: i32 = tail.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);
    sign * (h * 60 + m)
}

pub fn month_key(lat: f64, lng: f64, method: &str, madhab: &str, year: i32, month: u32) -> String {
    format!("{lat:.4}_{lng:.4}_{method}_{madhab}_{year:04}-{month:02}")
}

fn cache_path(key: &str) -> std::path::PathBuf {
    crate::settings::data_dir().join(format!("times-{key}.json"))
}

pub fn cached(key: &str) -> Option<Month> {
    serde_json::from_str(&std::fs::read_to_string(cache_path(key)).ok()?).ok()
}

/// Downloads one month and saves it. Runs on a worker thread.
pub fn fetch(lat: f64, lng: f64, method: &str, madhab: &str, year: i32, month: u32) -> Result<Month, String> {
    let url = format!("{API}/calendar/monthly?lat={lat}&lng={lng}&year={year}&month={month}&method={method}&madhab={madhab}");
    let resp: Resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let off = resp.data.days.first().map(|d| offset(&d.prayers.fajr.iso)).unwrap_or(0);
    let days = resp
        .data
        .days
        .into_iter()
        .map(|d| {
            let p = &d.prayers;
            Day {
                minutes: [p.fajr.iso.as_str(), &p.sunrise.iso, &p.dhuhr.iso, &p.asr.iso, &p.maghrib.iso, &p.isha.iso].map(minutes),
                hijri: d.hijri.trim_end_matches(" AH").to_string(),
                date: d.date,
            }
        })
        .collect();
    let m = Month { key: month_key(lat, lng, method, madhab, year, month), offset: off, days };
    let dir = crate::settings::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    // Drop older months for any city, then save this one.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("times-") && n != format!("times-{}.json", m.key) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if let Ok(s) = serde_json::to_string(&m) {
        let _ = std::fs::write(cache_path(&m.key), s);
    }
    Ok(m)
}

// ---------- calendar arithmetic (proleptic Gregorian, days since 1970-01-01) ----------

pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y } as i32, m, d)
}

/// 0 = Sunday … 6 = Saturday.
pub fn weekday(days: i64) -> u32 {
    ((days + 4).rem_euclid(7)) as u32
}

/// Seconds since 1970 in UTC, from the system clock.
pub fn utc_seconds() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The city's local "now": (days since 1970, second of the day).
pub fn local_now(offset_min: i32) -> (i64, u32) {
    let s = utc_seconds() + offset_min as i64 * 60;
    (s.div_euclid(86400), s.rem_euclid(86400) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_minutes() {
        assert_eq!(minutes("2026-10-14T04:40:00+06:00"), 280);
        assert_eq!(minutes("2026-10-14T18:44:00+06:00"), 1124);
    }

    #[test]
    fn parses_offsets() {
        assert_eq!(offset("2026-10-14T04:40:00+06:00"), 360);
        assert_eq!(offset("2026-10-14T04:40:00-04:30"), -270);
        assert_eq!(offset("2026-10-14T04:40:00Z"), 0);
    }

    #[test]
    fn calendar_round_trips() {
        let d = days_from_civil(2026, 10, 14);
        assert_eq!(civil_from_days(d), (2026, 10, 14));
        assert_eq!(weekday(d), 3); // Wednesday
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }
}
