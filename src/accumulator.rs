//! Section 5 of the paper: the gated exact accumulator.

use crate::ConfigError;

/// What a call to [`Accumulator::step`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Valid tick: the exact update of equation (6) was applied.
    Updated,
    /// Gated tick: the value was held.
    Held,
    /// The tick was declared valid but its data was malformed (negative or
    /// non-finite input, non-positive or non-finite time step). The value
    /// was held. Callers must treat the tick as invalid.
    Rejected,
}

/// Scalar leaky accumulator `z' = -alpha z + p` advanced by the exact
/// zero-order-hold recurrence of Definition 5.1:
///
/// `z_k = a z_{k-1} + b p_k`, `a = exp(-alpha dt)`, `b = (1 - a) / alpha`
/// on a valid tick, and `z_k = z_{k-1}` on a gated tick.
///
/// Guarantees (paper): `z >= 0` and `z <= max(z_0, P/alpha)` (Theorem 5.3),
/// monotone in input and initial value (Theorem 5.4), exact for held inputs
/// whatever the tick spacing (Lemma 5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accumulator {
    alpha: f64,
    z: f64,
    z_max: f64,
    saturated: bool,
}

impl Accumulator {
    /// Creates an accumulator with rate `alpha`, initial value `z0` and cap
    /// `z_max` (use `f64::INFINITY` for no cap).
    pub fn new(alpha: f64, z0: f64, z_max: f64) -> Result<Self, ConfigError> {
        if !(alpha > 0.0 && alpha.is_finite()) {
            return Err(ConfigError::Rate);
        }
        if !(z_max > 0.0) {
            return Err(ConfigError::Cap);
        }
        if !(z0 >= 0.0 && z0.is_finite() && z0 <= z_max) {
            return Err(ConfigError::InitialValue);
        }
        Ok(Self {
            alpha,
            z: z0,
            z_max,
            saturated: false,
        })
    }

    /// Current value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.z
    }

    /// Decay rate.
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Sticky flag: the cap was reached at least once (Proposition 5.10).
    #[must_use]
    pub fn saturated(&self) -> bool {
        self.saturated
    }

    /// Advances by one tick of length `dt` with sample `p`.
    ///
    /// `b` is computed as `-expm1(-alpha dt) / alpha`, which keeps full
    /// relative accuracy for small `alpha dt` (hypothesis of Proposition 5.8).
    pub fn step(&mut self, p: f64, dt: f64, valid: bool) -> StepOutcome {
        if !valid {
            return StepOutcome::Held;
        }
        if !Self::accepts(p, dt) {
            return StepOutcome::Rejected;
        }
        let x = -self.alpha * dt;
        let a = libm::exp(x);
        let b = -libm::expm1(x) / self.alpha;
        let mut z_new = a * self.z + b * p;
        // `!(z_new < z_max)` also catches an arithmetic overflow to infinity
        // when no finite cap is configured; the stored value stays finite.
        if !(z_new < self.z_max) {
            z_new = if self.z_max.is_finite() {
                self.z_max
            } else {
                f64::MAX
            };
            self.saturated = true;
        }
        self.z = z_new;
        StepOutcome::Updated
    }

    /// Returns true if `(p, dt)` is well formed for a valid tick.
    #[must_use]
    pub fn accepts(p: f64, dt: f64) -> bool {
        p >= 0.0 && p.is_finite() && dt > 0.0 && dt.is_finite()
    }
}
