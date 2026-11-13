//! Parameter validation errors.

use core::fmt;

/// A parameter violates a requirement of the specification (Appendix A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// A decay rate must be finite and strictly positive.
    Rate,
    /// An initial accumulator value must be finite, non-negative and at most `z_max`.
    InitialValue,
    /// The cap `z_max` must be strictly positive (it may be infinite).
    Cap,
    /// Thresholds must satisfy `0 <= z_exit < z_enter <= z_crit <= z_max`.
    Thresholds,
    /// The stability duration, cooldown, timeout or maximum tick must be finite and positive.
    Duration,
    /// The guard band must be finite and non-negative.
    GuardBand,
    /// `n_max` must satisfy `1 <= n_max <= M` (the engine capacity).
    AttemptLimit,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ConfigError::Rate => "decay rate must be finite and > 0",
            ConfigError::InitialValue => "initial value must be finite, >= 0 and <= z_max",
            ConfigError::Cap => "z_max must be > 0",
            ConfigError::Thresholds => "need 0 <= z_exit < z_enter <= z_crit <= z_max",
            ConfigError::Duration => "durations must be finite and > 0",
            ConfigError::GuardBand => "guard band must be finite and >= 0",
            ConfigError::AttemptLimit => "need 1 <= n_max <= capacity",
        };
        f.write_str(s)
    }
}
