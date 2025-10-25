"""The curated references the wallpaper draws from.

Only *references* live here. Every word of Arabic, English and Bangla is pulled from the
published datasets by build_content.py, never typed by hand:
- Quran: the `holy-quran` npm library (Hafs text, Sahih International English, Bangla).
- Hadith: fawazahmed0/hadith-api (public domain), with the scholars' gradings.
- Adhkar, duas, istighfar, durood: Hisn al-Muslim (hisnmuslim.com API), by item number.
"""

# (surah, first verse, last verse, tags). Short, complete verses that stand on their own.
QURAN = [
    (2, 152, 152, ["dhikr"]), (2, 153, 153, []), (2, 186, 186, ["dua"]), (2, 201, 201, ["dua"]),
    (2, 156, 156, []), (2, 45, 45, []), (3, 8, 8, ["dua"]), (3, 139, 139, []), (3, 173, 173, []),
    (3, 200, 200, []), (3, 31, 31, []), (7, 23, 23, ["dua", "istighfar"]), (13, 28, 28, ["dhikr"]),
    (14, 7, 7, []), (14, 40, 40, ["dua"]), (14, 41, 41, ["dua"]), (17, 24, 24, ["dua"]), (18, 10, 10, ["dua"]),
    (20, 25, 28, ["dua"]), (21, 87, 87, ["dua", "istighfar"]), (23, 118, 118, ["dua"]), (25, 74, 74, ["dua"]),
    (28, 24, 24, ["dua"]), (29, 69, 69, []), (33, 41, 42, ["dhikr"]), (33, 56, 56, ["durood"]),
    (39, 53, 53, ["istighfar"]), (40, 60, 60, ["dua"]), (47, 7, 7, []), (50, 16, 16, []), (51, 56, 56, []),
    (55, 13, 13, []), (6, 162, 162, []), (9, 129, 129, ["dhikr"]), (93, 5, 5, []), (94, 5, 6, []),
    (103, 1, 3, []), (112, 1, 4, []), (65, 3, 3, []), (11, 88, 88, []), (2, 286, 286, ["dua"]),
    (59, 10, 10, ["dua"]), (66, 8, 8, ["istighfar"]), (71, 10, 10, ["istighfar"]), (110, 3, 3, ["istighfar"]),
]

# (collection, number, tags). Muslim numbers are the standard ones (hadith-api `arabicnumber`).
HADITH = [
    ("bukhari", 1, []), ("bukhari", 6405, ["dhikr"]), ("bukhari", 6406, ["dhikr"]), ("bukhari", 6407, ["dhikr"]),
    ("bukhari", 6307, ["istighfar"]), ("bukhari", 6018, []), ("bukhari", 13, []), ("bukhari", 6412, []),
    ("bukhari", 6464, []), ("bukhari", 5027, []), ("bukhari", 1903, ["fasting"]), ("bukhari", 1901, ["ramadan"]),
    ("bukhari", 2017, ["ramadan"]), ("bukhari", 6011, []), ("bukhari", 3370, ["durood"]), ("bukhari", 6324, ["sleep"]),
    ("muslim", 2137, ["dhikr"]), ("muslim", 2691, ["dhikr"]), ("muslim", 1164, ["shawwal"]), ("muslim", 2564, []),
    ("muslim", 1631, []), ("muslim", 2588, []), ("muslim", 408, ["durood"]), ("muslim", 1162, ["arafah", "ashura"]),
    ("tirmidhi", 761, ["white"]), ("tirmidhi", 747, ["monthu"]), ("tirmidhi", 3383, ["dhikr"]),
    ("nawawi", 1, []), ("nawawi", 13, []), ("nawawi", 15, []), ("nawawi", 18, []), ("nawawi", 19, []),
    ("nawawi", 20, []), ("nawawi", 40, []),
]

# Hisn al-Muslim chapters (hisnmuslim.com ids) and when each is shown.
ADHKAR_CHAPTERS = {
    27: "morning_evening",  # Words of remembrance for morning and evening
    28: "sleep",            # What to say before sleeping
    1: "waking",            # Upon waking up
    25: "after_salah",      # After completing the prayer
    129: "istighfar",       # Repentance and seeking forgiveness
    130: "dhikr",           # The excellence of remembering Allah
    131: "dhikr",           # How the Prophet glorified Allah
    107: "durood",          # The excellence of blessings on the Prophet
    23: "durood",           # How to send blessings on the Prophet after the Tashahhud
    34: "dua",              # Worry and grief
    35: "dua",              # Anguish
    68: "iftar",            # Breaking the fast
    119: "arafah",          # The Day of Arafah
    67: "new_moon",         # Sighting the new moon
}
