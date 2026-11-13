//! Section 7.1 of the paper: warning flag with hysteresis.

use crate::ConfigError;

/// Hysteresis element of Definition 7.1.
///
/// The flag is set as soon as `z >= z_enter` on a valid tick and cleared
/// only after `z <= z_exit` has been observed on an uninterrupted run of
/// valid ticks lasting at least `t_stable`. A gated tick interrupts the run.
///
/// Guarantees (Theorem 7.3): `z >= z_enter` implies the flag is set, and two
/// consecutive entries are more than `t_stable + tau_rise` apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hysteresis {
    z_enter: f64,
    z_exit: f64,
    t_stable: f64,
    w: bool,
    below_since: Option<f64>,
}

impl Hysteresis {
    /// Creates the element. `w0` is the initial flag; Theorem 7.3 requires
    /// `w0 = true` unless the accumulator starts below `z_enter`.
    pub fn new(z_enter: f64, z_exit: f64, t_stable: f64, w0: bool) -> Result<Self, ConfigError> {
        if !(z_exit >= 0.0 && z_exit < z_enter && z_enter.is_finite()) {
            return Err(ConfigError::Thresholds);
        }
        if !(t_stable > 0.0 && t_stable.is_finite()) {
            return Err(ConfigError::Duration);
        }
        Ok(Self {
            z_enter,
            z_exit,
            t_stable,
            w: w0,
            below_since: None,
        })
    }

    /// Current flag.
    #[must_use]
    pub fn flag(&self) -> bool {
        self.w
    }

    /// Processes the accumulator value `z` observed at time `t`.
    pub fn step(&mut self, z: f64, t: f64, valid: bool) -> bool {
        if !valid {
            self.below_since = None;
            return self.w;
        }
        if z >= self.z_enter {
            self.w = true;
            self.below_since = None;
        } else if z <= self.z_exit {
            let since = *self.below_since.get_or_insert(t);
            if t - since >= self.t_stable {
                self.w = false;
            }
        } else {
            self.below_since = None;
        }
        self.w
    }
}
