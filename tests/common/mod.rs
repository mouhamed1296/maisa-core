//! Shared test helpers: a small deterministic generator (SplitMix64).
#![allow(dead_code)]

pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    /// Log-uniform in [lo, hi].
    pub fn log_uniform(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.unit()).exp()
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
    /// An arbitrary bit pattern: includes NaN, infinities, subnormals, negatives.
    pub fn any_f64(&mut self) -> f64 {
        match self.below(8) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::NEG_INFINITY,
            3 => 0.0,
            4 => -self.log_uniform(1e-300, 1e300),
            5 => self.log_uniform(1e-300, 1e300),
            _ => f64::from_bits(self.next_u64()),
        }
    }
}

/// Relative closeness used when comparing against the Python oracle, whose
/// exp/expm1 come from the C library while this crate uses the `libm` crate.
pub fn close(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()) + 1e-300
}
