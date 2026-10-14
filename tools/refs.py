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
    # added in 1.1.1
    (1, 1, 7, ["dua"]), (2, 255, 255, ["dhikr"]), (2, 285, 285, []), (2, 110, 110, []), (2, 115, 115, []),
    (2, 127, 127, ["dua"]), (2, 128, 128, ["dua"]), (2, 155, 155, []), (2, 183, 183, ["fasting"]), (2, 195, 195, []),
    (2, 216, 216, []), (2, 222, 222, []), (2, 238, 238, []), (2, 250, 250, ["dua"]), (2, 263, 263, []),
    (3, 16, 16, ["dua", "istighfar"]), (3, 26, 26, []), (3, 53, 53, ["dua"]), (3, 92, 92, []), (3, 102, 102, []),
    (3, 103, 103, []), (3, 134, 134, []), (3, 147, 147, ["dua", "istighfar"]), (3, 185, 185, []), (3, 190, 191, ["dhikr"]),
    (3, 193, 194, ["dua"]), (4, 28, 28, []), (4, 110, 110, ["istighfar"]), (4, 147, 147, []), (5, 8, 8, []),
    (6, 103, 103, []), (6, 160, 160, []), (7, 55, 56, ["dua"]), (7, 126, 126, ["dua"]), (7, 151, 151, ["dua", "istighfar"]),
    (7, 180, 180, ["dhikr"]), (7, 199, 199, []), (7, 204, 205, ["dhikr"]), (8, 2, 2, []), (8, 46, 46, []),
    (9, 51, 51, []), (9, 119, 119, []), (10, 57, 57, []), (10, 62, 62, []), (10, 85, 86, ["dua"]),
    (11, 6, 6, []), (11, 47, 47, ["dua", "istighfar"]), (11, 114, 115, []), (12, 86, 87, []), (12, 101, 101, ["dua"]),
    (13, 24, 24, []), (14, 34, 34, []), (15, 49, 49, []), (15, 98, 99, ["dhikr"]), (16, 18, 18, []),
    (16, 90, 90, []), (16, 97, 97, []), (16, 125, 125, []), (16, 128, 128, []), (17, 23, 23, []),
    (17, 80, 80, ["dua"]), (17, 82, 82, []), (18, 46, 46, []), (18, 109, 109, []), (20, 14, 14, []),
    (20, 114, 114, ["dua"]), (21, 83, 83, ["dua"]), (21, 89, 89, ["dua"]), (21, 107, 107, []), (22, 77, 77, []),
    (23, 1, 2, []), (23, 97, 98, ["dua"]), (23, 109, 109, ["dua"]), (25, 63, 63, []), (25, 65, 66, ["dua"]),
    (26, 83, 85, ["dua"]), (26, 87, 89, ["dua"]), (27, 19, 19, ["dua"]), (27, 62, 62, []), (29, 45, 45, []),
    (29, 57, 57, []), (30, 21, 21, []), (31, 17, 17, []), (31, 18, 18, []), (33, 70, 71, []),
    (35, 15, 15, []), (35, 34, 34, []), (39, 9, 9, []), (39, 10, 10, []), (40, 44, 44, []),
    (41, 30, 30, []), (41, 34, 34, []), (42, 30, 30, []), (42, 43, 43, []), (49, 10, 10, []),
    (49, 13, 13, []), (50, 18, 18, []), (51, 55, 55, []), (53, 39, 41, []), (55, 26, 27, []),
    (55, 60, 60, []), (59, 18, 18, []), (59, 22, 24, ["dhikr"]), (62, 9, 10, []), (63, 9, 9, []),
    (64, 16, 16, []), (67, 1, 2, []), (76, 8, 9, []), (87, 14, 15, []), (89, 27, 30, []),
    (93, 1, 11, []), (94, 1, 4, []), (94, 7, 8, []), (97, 1, 5, ["ramadan"]), (99, 7, 8, []),
    (102, 1, 8, []), (108, 1, 3, []), (109, 1, 6, []), (113, 1, 5, ["dua"]), (114, 1, 6, ["dua"]),
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
    # added in 1.1.1, picked from Bukhari's books of Faith, Manners, Invocations and Softening the Heart,
    # Muslim's books of Faith, Virtue, Remembrance and Zuhd, and the rest of an-Nawawi's Forty
    *[("bukhari", n, []) for n in (9, 10, 14, 35, 42, 55, 444, 481, 5986, 5991, 6005, 6012, 6013, 6014, 6057, 6058,
                                    6065, 6114, 6116, 6125, 6133, 6136, 6170, 6338, 6340, 6418, 6421, 6439, 6446,
                                    6474, 6485, 6487, 6488, 6490, 6508, 6552)],
    *[("muslim", n, []) for n in (26, 34, 35, 41, 55, 2552, 2555, 2557, 2566, 2568, 2572, 2573, 2579, 2585, 2586,
                                   2590, 2592, 2609, 2626, 2675, 2683, 2694, 2695, 2702, 2703, 2732, 2734, 2735,
                                   2751, 2753, 2956, 2983, 2988)],
    *[("nawawi", n, []) for n in range(2, 43) if n not in (13, 15, 18, 19, 20, 40)],
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
    # added in 1.1.1: everyday duas, shown in the Dua, Dhikr and Istighfar turns
    10: "dua",   # Leaving the home
    11: "dua",   # Entering the home
    12: "dua",   # Going to the mosque
    13: "dua",   # Entering the mosque
    14: "dua",   # Leaving the mosque
    15: "dua",   # Hearing the athan
    24: "dua",   # After the final tashahhud, before ending the prayer
    33: "dhikr",  # After the witr prayer
    41: "dua",   # Settling a debt
    43: "dua",   # When something becomes difficult
    44: "istighfar",  # After committing a sin
    45: "dua",   # Against the devil's promptings
    46: "dua",   # When something you dislike happens
    53: "dua",   # When tragedy strikes
    61: "dua",   # When the wind blows
    62: "dua",   # Thunder
    64: "dua",   # When it rains
    65: "dua",   # After rain
    69: "dua",   # Before eating
    70: "dua",   # After eating
    82: "dua",   # Anger
    85: "dhikr",  # Expiation of the assembly
    92: "dua",   # Fear of shirk
    95: "dua",   # Riding a vehicle
    96: "dua",   # Travelling
    105: "dua",  # Returning from a journey
    122: "dhikr",  # When surprised
    123: "dhikr",  # When something pleases you
    124: "dua",  # Pain in the body
    126: "dua",  # When frightened
}
