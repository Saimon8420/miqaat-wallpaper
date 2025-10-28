//! The picture's light through the day: a gentle colour wash that follows the prayers and a
//! darkness at night the user controls.

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Light {
    /// Black laid over the photo, 0..1.
    pub dark: f32,
    /// Colour wash (r, g, b, alpha).
    pub tint: [f32; 4],
}

/// Key moments as (minute, share of night darkness, tint). Small alphas on purpose: Dhuhr and
/// Asr only change "a little bit", the night is where most of the change happens.
fn keys(t: &[u32; 6]) -> [(i32, f32, [f32; 4]); 10] {
    let [fajr, sunrise, dhuhr, asr, maghrib, isha] = t.map(|v| v as i32);
    const NIGHT: [f32; 4] = [0.05, 0.08, 0.20, 0.16];
    [
        (fajr - 40, 1.0, NIGHT),
        (fajr, 0.6, [0.85, 0.45, 0.58, 0.12]),       // soft rose at Fajr
        (sunrise, 0.12, [1.0, 0.72, 0.35, 0.13]),    // gold at sunrise
        (sunrise + 60, 0.0, [1.0, 0.85, 0.6, 0.03]),
        (dhuhr, 0.0, [1.0, 1.0, 1.0, 0.0]),          // bright at Dhuhr
        (asr, 0.0, [1.0, 0.78, 0.48, 0.06]),         // warm at Asr
        (maghrib - 30, 0.05, [1.0, 0.62, 0.32, 0.11]),
        (maghrib, 0.3, [1.0, 0.5, 0.26, 0.15]),      // orange at Maghrib
        (isha, 0.8, [0.18, 0.28, 0.62, 0.15]),       // blue at Isha
        (isha + 60, 1.0, NIGHT),
    ]
}

/// Light at minute `now` of the day. `night` is the user's night darkness (0..1); when the
/// light doesn't follow the sky only the night darkness is applied.
pub fn at(now: u32, t: &[u32; 6], night: f32, follow: bool) -> Light {
    let k = keys(t);
    let now = now as i32;
    // Find the pair of keys around `now`, wrapping past midnight.
    let mut prev = (k[9].0 - 1440, k[9].1, k[9].2);
    let mut next = k[0];
    for i in 0..k.len() {
        if now < k[i].0 {
            next = k[i];
            break;
        }
        prev = k[i];
        next = if i + 1 < k.len() { k[i + 1] } else { (k[0].0 + 1440, k[0].1, k[0].2) };
    }
    let span = (next.0 - prev.0).max(1) as f32;
    let f = ((now - prev.0) as f32 / span).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f); // ease, so the change is never a visible step
    let lerp = |a: f32, b: f32| a + (b - a) * f;
    let dark = lerp(prev.1, next.1) * night * 0.78;
    let tint = if follow {
        [lerp(prev.2[0], next.2[0]), lerp(prev.2[1], next.2[1]), lerp(prev.2[2], next.2[2]), lerp(prev.2[3], next.2[3])]
    } else {
        [0.0; 4]
    };
    Light { dark, tint }
}

impl Light {
    /// Rounded so a repaint of the whole picture only happens when the change is visible.
    pub fn key(&self) -> [i32; 5] {
        let q = |v: f32| (v * 120.0).round() as i32;
        [q(self.dark), q(self.tint[0]), q(self.tint[1]), q(self.tint[2]), q(self.tint[3])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: [u32; 6] = [280, 355, 710, 950, 1065, 1140];

    #[test]
    fn day_is_clear_and_night_is_dark() {
        let noon = at(720, &T, 1.0, true);
        assert!(noon.dark < 0.01 && noon.tint[3] < 0.02);
        let night = at(1380, &T, 1.0, true);
        assert!(night.dark > 0.7);
        let midnight = at(10, &T, 1.0, true);
        assert!(midnight.dark > 0.7);
        assert_eq!(at(1380, &T, 0.0, true).dark, 0.0);
    }

    #[test]
    fn asr_changes_only_a_little() {
        let asr = at(950, &T, 1.0, true);
        assert!(asr.tint[3] > 0.0 && asr.tint[3] < 0.1 && asr.dark < 0.02);
    }
}
