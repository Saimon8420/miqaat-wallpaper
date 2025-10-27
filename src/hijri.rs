//! Hijri date from the API's text, the user's ±2 day adjustment, and the special days.
//!
//! Special days are only those with clear evidence; each carries its reference. Disputed
//! days (for example 15 Sha'ban or the Mawlid) are deliberately not presented as worship days.

use crate::i18n::Lang;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hijri {
    pub day: u32,
    /// 1 = Muharram … 12 = Dhu al-Hijjah
    pub month: u32,
    pub year: u32,
}

/// "3 Jumada al-Awwal 1448" -> Hijri { 3, 5, 1448 }.
pub fn parse(s: &str) -> Option<Hijri> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let day = parts.first()?.parse().ok()?;
    let year = parts.last()?.parse().ok()?;
    let name = parts[1..parts.len() - 1].join(" ").to_lowercase();
    let month = if name.contains("muharram") {
        1
    } else if name.contains("safar") {
        2
    } else if name.contains("rabi") {
        if name.contains("awwal") { 3 } else { 4 }
    } else if name.contains("jumad") {
        if name.contains("awwal") || name.contains("ula") { 5 } else { 6 }
    } else if name.contains("rajab") {
        7
    } else if name.contains("sha") && name.contains("ban") {
        8
    } else if name.contains("ramad") {
        9
    } else if name.contains("shaww") {
        10
    } else if name.contains("qi") || name.contains("qa") {
        11
    } else if name.contains("hij") {
        12
    } else {
        return None;
    };
    Some(Hijri { day, month, year })
}

impl Hijri {
    /// Moves by whole days, treating every month as 30 days long. Only used for the ±2 day
    /// adjustment and "tomorrow", where that small error at a month's end is acceptable.
    pub fn shift(self, by: i32) -> Hijri {
        let idx = (self.year as i64 * 12 + self.month as i64 - 1) * 30 + self.day as i64 - 1 + by as i64;
        let (my, d) = (idx.div_euclid(30), idx.rem_euclid(30) as u32 + 1);
        Hijri { day: d, month: (my.rem_euclid(12) + 1) as u32, year: my.div_euclid(12) as u32 }
    }

    pub fn text(&self, l: Lang) -> String {
        let s = match l {
            Lang::Ar => format!("{} {} {} هـ", self.day, crate::i18n::hijri_month(l, self.month), self.year),
            _ => format!("{} {} {}", self.day, crate::i18n::hijri_month(l, self.month), self.year),
        };
        crate::i18n::digits(l, false, &s)
    }
}

pub struct Special {
    pub text: String,
    pub reference: &'static str,
    /// Fasting-related days get a heads-up notification the evening before.
    pub heads_up: bool,
}

#[allow(overlapping_range_endpoints)] // Tashriq (13 Dhu al-Hijjah) deliberately wins over the White Days
/// The most important special day for a Hijri date and weekday (0 = Sunday), if any.
pub fn special(h: Hijri, weekday: u32, l: Lang) -> Option<Special> {
    let t = |en: &str, bn: &str, ar: &str| match l {
        Lang::En => en.to_string(),
        Lang::Bn => bn.to_string(),
        Lang::Ar => ar.to_string(),
    };
    let n = crate::i18n::digits(l, false, &h.day.to_string());
    let (text, reference, heads_up) = match (h.month, h.day) {
        (10, 1) => (t("Eid al-Fitr · no fasting today", "ঈদুল ফিতর · আজ রোজা রাখা নিষেধ", "عيد الفطر · لا صيام اليوم"), "Sahih al-Bukhari 1990", false),
        (12, 10) => (t("Eid al-Adha · no fasting today", "ঈদুল আযহা · আজ রোজা রাখা নিষেধ", "عيد الأضحى · لا صيام اليوم"), "Sahih al-Bukhari 1990", false),
        (12, 9) => (
            t("Day of Arafah · fasting today wipes out two years of sins", "আরাফার দিন · আজকের রোজা দুই বছরের গুনাহ মাফের কারণ", "يوم عرفة · صيامه يكفّر سنتين"),
            "Sahih Muslim 1162",
            true,
        ),
        (1, 10) => (
            t("Ashura · fasting today wipes out the past year's sins", "আশুরা · আজকের রোজা আগের এক বছরের গুনাহ মাফের কারণ", "عاشوراء · صيامه يكفّر السنة الماضية"),
            "Sahih Muslim 1162",
            true,
        ),
        (1, 9) => (t("Tasu'a · fast the 9th along with Ashura", "৯ মুহাররম · আশুরার সাথে আজও রোজা রাখুন", "تاسوعاء · صُم التاسع مع العاشر"), "Sahih Muslim 1134", true),
        (12, 11..=13) => (
            t("Days of Tashriq · days of eating, drinking and remembering Allah", "আইয়ামে তাশরীক · খাওয়া-দাওয়া আর আল্লাহর যিকিরের দিন", "أيام التشريق · أيام أكل وشرب وذكر لله"),
            "Sahih Muslim 1141",
            false,
        ),
        (9, 21..=30) => (
            t("Last ten nights · seek Laylat al-Qadr in the odd nights", "রমজানের শেষ দশক · বিজোড় রাতে শবে কদর খুঁজুন", "العشر الأواخر · التمسوا ليلة القدر في الوتر"),
            "Sahih al-Bukhari 2017",
            false,
        ),
        (9, _) => (t(&format!("Ramadan · day {n}"), &format!("রমজান · {n} তম রোজা"), &format!("رمضان · اليوم {n}")), "Sahih al-Bukhari 1901", false),
        (12, 1..=8) => (
            t("First ten days of Dhu al-Hijjah · the best days for good deeds", "জিলহজের প্রথম দশক · নেক আমলের শ্রেষ্ঠ দিন", "العشر من ذي الحجة · أفضل أيام العمل الصالح"),
            "Sahih al-Bukhari 969",
            false,
        ),
        (_, 13..=15) => (
            t("White Days · fasting the 13th, 14th and 15th is sunnah", "আইয়ামে বীজ · চাঁদের ১৩, ১৪, ১৫ তারিখ রোজা রাখা সুন্নত", "الأيام البيض · صيام ١٣ و١٤ و١٥ سنة"),
            "Jami' at-Tirmidhi 761",
            true,
        ),
        _ => match weekday {
            1 => (t("Monday · fasting today is sunnah", "সোমবার · আজ রোজা রাখা সুন্নত", "الاثنين · يُسنّ صيامه"), "Jami' at-Tirmidhi 747", true),
            4 => (t("Thursday · fasting today is sunnah", "বৃহস্পতিবার · আজ রোজা রাখা সুন্নত", "الخميس · يُسنّ صيامه"), "Jami' at-Tirmidhi 747", true),
            5 => (
                t("Friday · read Surah al-Kahf and send durood often", "জুমার দিন · সূরা কাহফ পড়ুন, বেশি বেশি দরুদ পড়ুন", "يوم الجمعة · سورة الكهف والإكثار من الصلاة على النبي ﷺ"),
                "Sunan Abi Dawud 1047 · Sahih al-Jami' 6470",
                false,
            ),
            _ => match h.month {
                10 => (t("Shawwal · fast six days of this month", "শাওয়াল · এই মাসে ছয়টি রোজা রাখুন", "شوال · صيام ستة أيام منه"), "Sahih Muslim 1164", false),
                1 => (t("Muharram · the best fasting after Ramadan", "মুহাররম · রমজানের পর সবচেয়ে ফজিলতের রোজা", "المحرم · أفضل الصيام بعد رمضان"), "Sahih Muslim 1163", false),
                8 => (
                    t("Sha'ban · the Prophet ﷺ fasted most in this month", "শাবান · নবীজি ﷺ এই মাসে সবচেয়ে বেশি রোজা রাখতেন", "شعبان · كان النبي ﷺ يكثر الصيام فيه"),
                    "Sahih al-Bukhari 1969",
                    false,
                ),
                _ => return None,
            },
        },
    };
    Some(Special { text, reference, heads_up })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_text() {
        assert_eq!(parse("3 Jumada al-Awwal 1448"), Some(Hijri { day: 3, month: 5, year: 1448 }));
        assert_eq!(parse("20 Rabi' al-Thani 1448"), Some(Hijri { day: 20, month: 4, year: 1448 }));
        assert_eq!(parse("1 Dhu al-Hijjah 1447"), Some(Hijri { day: 1, month: 12, year: 1447 }));
        assert_eq!(parse("9 Dhu al-Qi'dah 1447"), Some(Hijri { day: 9, month: 11, year: 1447 }));
        assert_eq!(parse("2 Sha'ban 1447"), Some(Hijri { day: 2, month: 8, year: 1447 }));
        assert_eq!(parse("2 Shawwal 1447"), Some(Hijri { day: 2, month: 10, year: 1447 }));
    }

    #[test]
    fn shifts_across_months() {
        let h = Hijri { day: 1, month: 1, year: 1448 };
        assert_eq!(h.shift(-1), Hijri { day: 30, month: 12, year: 1447 });
        assert_eq!(Hijri { day: 30, month: 9, year: 1447 }.shift(1), Hijri { day: 1, month: 10, year: 1447 });
    }

    #[test]
    fn picks_strongest_day() {
        let arafah = special(Hijri { day: 9, month: 12, year: 1447 }, 1, Lang::En).unwrap();
        assert_eq!(arafah.reference, "Sahih Muslim 1162");
        // White Days outrank Monday; plain Wednesday in Rajab has nothing.
        assert_eq!(special(Hijri { day: 14, month: 5, year: 1448 }, 1, Lang::En).unwrap().reference, "Jami' at-Tirmidhi 761");
        assert!(special(Hijri { day: 3, month: 7, year: 1448 }, 3, Lang::En).is_none());
    }
}
