//! Section 3 of the paper: deviation and response.

/// Normalised distance of `y` to the interval `[lo, hi]` (Definition 3.1).
///
/// Returns `max(lo - y, 0, y - hi) / s`. By Lemma 3.2 the result is
/// non-negative, zero exactly inside the interval and `1/s`-Lipschitz in `y`.
/// A NaN observation yields NaN, which [`crate::Monitor`] treats as invalid.
#[must_use]
pub fn deviation(y: f64, lo: f64, hi: f64, s: f64) -> f64 {
    if y < lo {
        (lo - y) / s
    } else if y > hi {
        (y - hi) / s
    } else if y >= lo && y <= hi {
        0.0
    } else {
        f64::NAN
    }
}

/// Response `max(0, d - deadband)^power`, a member of the class of
/// Definition 3.3 for every `power >= 1`.
#[must_use]
pub fn response(d: f64, deadband: f64, power: f64) -> f64 {
    let e = d - deadband;
    if e > 0.0 {
        if power == 1.0 {
            e
        } else {
            libm::pow(e, power)
        }
    } else if e <= 0.0 {
        0.0
    } else {
        f64::NAN
    }
}
