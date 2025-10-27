//! Words on the wallpaper in English, Bangla and Arabic.

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    En,
    Bn,
    Ar,
}

impl Lang {
    pub fn from(code: &str) -> Self {
        match code {
            "bn" => Lang::Bn,
            "ar" => Lang::Ar,
            _ => Lang::En,
        }
    }
}

pub fn prayer(l: Lang, i: usize) -> &'static str {
    const EN: [&str; 6] = ["Fajr", "Sunrise", "Dhuhr", "Asr", "Maghrib", "Isha"];
    const BN: [&str; 6] = ["ফজর", "সূর্যোদয়", "যোহর", "আসর", "মাগরিব", "এশা"];
    const AR: [&str; 6] = ["الفجر", "الشروق", "الظهر", "العصر", "المغرب", "العشاء"];
    match l {
        Lang::En => EN[i],
        Lang::Bn => BN[i],
        Lang::Ar => AR[i],
    }
}

/// Hijri month name, 1 = Muharram.
pub fn hijri_month(l: Lang, m: u32) -> &'static str {
    const EN: [&str; 12] = [
        "Muharram", "Safar", "Rabi' al-Awwal", "Rabi' al-Thani", "Jumada al-Awwal", "Jumada al-Thani",
        "Rajab", "Sha'ban", "Ramadan", "Shawwal", "Dhu al-Qi'dah", "Dhu al-Hijjah",
    ];
    const BN: [&str; 12] = [
        "মুহাররম", "সফর", "রবিউল আউয়াল", "রবিউস সানি", "জমাদিউল আউয়াল", "জমাদিউস সানি",
        "রজব", "শাবান", "রমজান", "শাওয়াল", "জিলকদ", "জিলহজ",
    ];
    const AR: [&str; 12] = [
        "محرم", "صفر", "ربيع الأول", "ربيع الآخر", "جمادى الأولى", "جمادى الآخرة",
        "رجب", "شعبان", "رمضان", "شوال", "ذو القعدة", "ذو الحجة",
    ];
    let i = (m.clamp(1, 12) - 1) as usize;
    match l {
        Lang::En => EN[i],
        Lang::Bn => BN[i],
        Lang::Ar => AR[i],
    }
}

/// Swaps 0-9 for Bangla or Arabic-Indic digits.
pub fn digits(l: Lang, arabic_digits: bool, s: &str) -> String {
    let base = if arabic_digits || l == Lang::Ar {
        Some('٠')
    } else if l == Lang::Bn {
        Some('০')
    } else {
        None
    };
    match base {
        None => s.to_string(),
        Some(z) => s
            .chars()
            .map(|c| if c.is_ascii_digit() { char::from_u32(z as u32 + (c as u32 - '0' as u32)).unwrap_or(c) } else { c })
            .collect(),
    }
}

pub fn ampm(l: Lang, pm: bool) -> &'static str {
    match (l, pm) {
        (Lang::En, false) => "am",
        (Lang::En, true) => "pm",
        (Lang::Bn, _) => "",
        (Lang::Ar, false) => "ص",
        (Lang::Ar, true) => "م",
    }
}

/// "Asr in 1h 05m" (or "in 12m" when under an hour).
pub fn next_in(l: Lang, name: &str, mins: u32) -> String {
    let (h, m) = (mins / 60, mins % 60);
    match l {
        Lang::En if h > 0 => format!("{name} in {h}h {m:02}m"),
        Lang::En => format!("{name} in {m}m"),
        Lang::Bn if h > 0 => format!("{name} বাকি {h} ঘণ্টা {m} মিনিট"),
        Lang::Bn => format!("{name} বাকি {m} মিনিট"),
        Lang::Ar if h > 0 => format!("{name} بعد {h} س {m} د"),
        Lang::Ar => format!("{name} بعد {m} د"),
    }
}

/// Label above the reminder.
pub fn kind(l: Lang, k: &str) -> &'static str {
    let i = ["quran", "hadith", "dua", "dhikr", "istighfar", "durood", "morning", "evening", "after_salah", "sleep", "waking", "iftar", "arafah", "new_moon"]
        .iter()
        .position(|x| *x == k)
        .unwrap_or(2);
    const EN: [&str; 14] = [
        "Quran", "Hadith", "Dua", "Dhikr", "Istighfar", "Durood", "Morning adhkar", "Evening adhkar", "After salah", "Before sleep",
        "On waking", "Iftar", "Day of Arafah", "New moon",
    ];
    const BN: [&str; 14] = [
        "কুরআন", "হাদিস", "দোয়া", "যিকির", "ইস্তিগফার", "দরুদ", "সকালের যিকির", "সন্ধ্যার যিকির", "নামাজের পর", "ঘুমের আগে",
        "ঘুম থেকে উঠে", "ইফতার", "আরাফার দিন", "নতুন চাঁদ",
    ];
    const AR: [&str; 14] = [
        "القرآن", "حديث", "دعاء", "ذكر", "استغفار", "الصلاة على النبي ﷺ", "أذكار الصباح", "أذكار المساء", "بعد الصلاة", "قبل النوم",
        "عند الاستيقاظ", "الإفطار", "يوم عرفة", "رؤية الهلال",
    ];
    match l {
        Lang::En => EN[i],
        Lang::Bn => BN[i],
        Lang::Ar => AR[i],
    }
}

/// Short phrases used in notifications and the "unavailable" state.
pub fn phrase(l: Lang, key: &str) -> &'static str {
    match (key, l) {
        ("time_for", Lang::En) => "It's time for",
        ("time_for", Lang::Bn) => "এখন সময়",
        ("time_for", Lang::Ar) => "حان وقت",
        ("tomorrow", Lang::En) => "Tomorrow",
        ("tomorrow", Lang::Bn) => "আগামীকাল",
        ("tomorrow", Lang::Ar) => "غدًا",
        ("loading", Lang::En) => "Loading prayer times…",
        ("loading", Lang::Bn) => "নামাজের সময় আসছে…",
        ("loading", Lang::Ar) => "جارٍ تحميل المواقيت…",
        ("offline", Lang::En) => "Prayer times unavailable · will retry",
        ("offline", Lang::Bn) => "নামাজের সময় পাওয়া যায়নি · আবার চেষ্টা হবে",
        ("offline", Lang::Ar) => "تعذّر تحميل المواقيت · ستُعاد المحاولة",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_digits() {
        assert_eq!(digits(Lang::Bn, false, "4:40"), "৪:৪০");
        assert_eq!(digits(Lang::En, true, "12"), "١٢");
        assert_eq!(digits(Lang::En, false, "12"), "12");
    }
}
