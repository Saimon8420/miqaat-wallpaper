//! Small extras on the panel: the Qibla direction, the countdown wording, fasting times,
//! the next Islamic event and the Gregorian date. All of it is plain arithmetic on data the app
//! already has, so none of it costs memory or CPU worth measuring.

use crate::i18n::{digits, Lang};

/// Direction of the Kaaba from `lat`, `lng`, in degrees clockwise from north.
pub fn qibla(lat: f64, lng: f64) -> f64 {
    let (kl, kg) = (21.4225_f64.to_radians(), 39.8262_f64.to_radians());
    let (la, lg) = (lat.to_radians(), lng.to_radians());
    let d = kg - lg;
    let b = d.sin().atan2(la.cos() * kl.tan() - la.sin() * d.cos());
    (b.to_degrees() + 360.0) % 360.0
}

fn compass_point(deg: f64) -> &'static str {
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][(((deg + 22.5) % 360.0) / 45.0) as usize]
}

pub fn qibla_text(l: Lang, deg: f64) -> String {
    let d = digits(l, false, &format!("{:.0}", deg));
    match l {
        Lang::En => format!("Qibla {d}° {}", compass_point(deg)),
        Lang::Bn => format!("কিবলা {d}°"),
        Lang::Ar => format!("القبلة {d}°"),
    }
}

/// "2h 10m" or "10m".
pub fn duration(l: Lang, mins: u32) -> String {
    let (h, m) = (mins / 60, mins % 60);
    let s = match l {
        Lang::En if h > 0 => format!("{h}h {m:02}m"),
        Lang::En => format!("{m}m"),
        Lang::Bn if h > 0 => format!("{h} ঘণ্টা {m} মিনিট"),
        Lang::Bn => format!("{m} মিনিট"),
        Lang::Ar if h > 0 => format!("{h} س {m} د"),
        Lang::Ar => format!("{m} د"),
    };
    digits(l, false, &s)
}

/// The line under the clock, in the chosen style: "in" (Asr in 1h 05m), "at" (Asr 3:04 pm ·
/// in 1h 05m) or "timer" (1:05 to Asr).
pub fn countdown(l: Lang, style: &str, name: &str, mins: u32, at: &str) -> String {
    let s = match (style, l) {
        ("at", Lang::En) => format!("{name} {at} · in {}", duration(l, mins)),
        ("at", Lang::Bn) => format!("{name} {at} · বাকি {}", duration(l, mins)),
        ("at", Lang::Ar) => format!("{name} {at} · بعد {}", duration(l, mins)),
        ("timer", Lang::En) => format!("{}:{:02} to {name}", mins / 60, mins % 60),
        ("timer", Lang::Bn) => format!("{name} বাকি {}:{:02}", mins / 60, mins % 60),
        ("timer", Lang::Ar) => format!("{name} بعد {}:{:02}", mins / 60, mins % 60),
        _ => crate::i18n::next_in(l, name, mins),
    };
    digits(l, false, &s)
}

pub fn gregorian(l: Lang, y: i32, m: u32, d: u32) -> String {
    const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const BN: [&str; 12] = ["জানুয়ারি", "ফেব্রুয়ারি", "মার্চ", "এপ্রিল", "মে", "জুন", "জুলাই", "আগস্ট", "সেপ্টেম্বর", "অক্টোবর", "নভেম্বর", "ডিসেম্বর"];
    const AR: [&str; 12] = ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"];
    let i = (m.clamp(1, 12) - 1) as usize;
    let name = match l {
        Lang::En => EN[i],
        Lang::Bn => BN[i],
        Lang::Ar => AR[i],
    };
    digits(l, false, &format!("{d} {name} {y}"))
}

/// "Iftar in 2h 10m · 5:17 pm", "Suhoor ends in 40m · 4:39 am", "Tomorrow: suhoor ends 4:39 am".
pub fn fasting(l: Lang, kind: &str, mins: Option<u32>, at: &str) -> String {
    let s = match (kind, l, mins) {
        ("iftar", Lang::En, Some(m)) => format!("Iftar in {} · {at}", duration(l, m)),
        ("iftar", Lang::Bn, Some(m)) => format!("ইফতার বাকি {} · {at}", duration(l, m)),
        ("iftar", Lang::Ar, Some(m)) => format!("الإفطار بعد {} · {at}", duration(l, m)),
        ("suhoor", Lang::En, Some(m)) => format!("Suhoor ends in {} · {at}", duration(l, m)),
        ("suhoor", Lang::Bn, Some(m)) => format!("সাহরির শেষ সময় বাকি {} · {at}", duration(l, m)),
        ("suhoor", Lang::Ar, Some(m)) => format!("ينتهي السحور بعد {} · {at}", duration(l, m)),
        (_, Lang::En, _) => format!("Tomorrow: suhoor ends {at}"),
        (_, Lang::Bn, _) => format!("আগামীকাল সাহরির শেষ সময় {at}"),
        (_, Lang::Ar, _) => format!("غدًا ينتهي السحور {at}"),
    };
    digits(l, false, &s)
}

/// The nearest of Ramadan, Eid al-Fitr and Eid al-Adha: "Ramadan in 95 days".
pub fn event(l: Lang, days_until: [i64; 3]) -> Option<String> {
    let names = match l {
        Lang::En => ["Ramadan", "Eid al-Fitr", "Eid al-Adha"],
        Lang::Bn => ["রমজান", "ঈদুল ফিতর", "ঈদুল আযহা"],
        Lang::Ar => ["رمضان", "عيد الفطر", "عيد الأضحى"],
    };
    let (i, d) = days_until.iter().enumerate().filter(|(_, d)| **d > 0).min_by_key(|(_, d)| **d)?;
    let n = names[i];
    let s = match (l, *d) {
        (Lang::En, 1) => format!("{n} is tomorrow"),
        (Lang::En, d) => format!("{n} in {d} days"),
        (Lang::Bn, 1) => format!("{n} আগামীকাল"),
        (Lang::Bn, d) => format!("{n} আর {d} দিন"),
        (Lang::Ar, 1) => format!("{n} غدًا"),
        (Lang::Ar, d) => format!("{n} بعد {d} يومًا"),
    };
    Some(digits(l, false, &s))
}

/// Notification texts: a few minutes before a prayer, and before Jumu'ah.
pub fn before_prayer(l: Lang, name: &str, mins: u32) -> String {
    let s = match l {
        Lang::En => format!("{name} in {mins} minutes"),
        Lang::Bn => format!("{name} বাকি {mins} মিনিট"),
        Lang::Ar => format!("{name} بعد {mins} دقيقة"),
    };
    digits(l, false, &s)
}

pub fn jumuah(l: Lang, mins: u32) -> (String, String) {
    match l {
        Lang::En => (format!("Jumu'ah in {mins} minutes"), "Read Surah al-Kahf and send durood often".into()),
        Lang::Bn => (digits(l, false, &format!("জুমা বাকি {mins} মিনিট")), "সূরা কাহফ পড়ুন, বেশি বেশি দরুদ পড়ুন".into()),
        Lang::Ar => (digits(l, false, &format!("الجمعة بعد {mins} دقيقة")), "اقرأ سورة الكهف وأكثر من الصلاة على النبي ﷺ".into()),
    }
}

/// "1.1.10" > "1.1.9".
pub fn newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| v.trim_start_matches('v').split('.').map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(remote) > parse(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qibla_from_known_cities() {
        // Dhaka points west-north-west, London south-east, New York north-east.
        assert!((qibla(23.81, 90.41) - 277.5).abs() < 1.5, "{}", qibla(23.81, 90.41));
        assert!((qibla(51.507, -0.128) - 119.0).abs() < 1.5, "{}", qibla(51.507, -0.128));
        assert!((qibla(40.71, -74.0) - 58.5).abs() < 1.5, "{}", qibla(40.71, -74.0));
    }

    #[test]
    fn countdown_styles_and_versions() {
        assert_eq!(countdown(Lang::En, "in", "Asr", 65, "3:04 pm"), "Asr in 1h 05m");
        assert_eq!(countdown(Lang::En, "at", "Asr", 65, "3:04 pm"), "Asr 3:04 pm · in 1h 05m");
        assert_eq!(countdown(Lang::En, "timer", "Asr", 65, "3:04 pm"), "1:05 to Asr");
        assert_eq!(event(Lang::En, [95, 124, 192]).unwrap(), "Ramadan in 95 days");
        assert_eq!(event(Lang::En, [0, 1, 70]).unwrap(), "Eid al-Fitr is tomorrow");
        assert!(newer("v1.1.10", "1.1.9") && !newer("v1.1.2", "1.1.2") && !newer("1.0.9", "1.1.0"));
    }
}
