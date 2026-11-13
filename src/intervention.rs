//! Section 7.3 of the paper: bounded intervention.

use crate::ConfigError;

/// Result of submitting a request (Definition 7.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    /// Rule 5: the request was accepted and is now pending.
    Accepted = 0,
    /// Rule 1: the identifier was already accepted since the last reset.
    Duplicate = 1,
    /// Rule 2: the authorisation guard is false.
    RejectedGuard = 2,
    /// Rule 3: the attempt budget is exhausted; escalation is raised.
    RejectedLimit = 3,
    /// Rule 4: the cooldown since the last accepted request has not elapsed.
    RejectedCooldown = 4,
}

/// Intervention engine of Definition 7.13 with room for `M` attempts.
///
/// Guarantees (Theorem 7.14): at most `n_max` accepted requests between two
/// authorised resets; accepted requests at least `t_cooldown` apart, also
/// across resets; no identifier accepted twice between resets; every accepted
/// request closed within `t_timeout` plus one tick; no retry once the budget
/// is exhausted, the condition being escalated instead.
///
/// The engine needs no allocation: since at most `n_max <= M` requests are
/// accepted between resets, `M` slots hold every identifier ever seen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterventionEngine<const M: usize> {
    n_max: usize,
    t_cooldown: f64,
    t_timeout: f64,
    n: usize,
    t_last: f64,
    seen: [u32; M],
    /// Acceptance time of each seen identifier while it is pending.
    pending: [Option<f64>; M],
    escalate: bool,
}

impl<const M: usize> InterventionEngine<M> {
    /// Creates an engine. Requires `1 <= n_max <= M`.
    pub fn new(n_max: usize, t_cooldown: f64, t_timeout: f64) -> Result<Self, ConfigError> {
        if n_max < 1 || n_max > M {
            return Err(ConfigError::AttemptLimit);
        }
        if !(t_cooldown > 0.0 && t_cooldown.is_finite() && t_timeout > 0.0 && t_timeout.is_finite())
        {
            return Err(ConfigError::Duration);
        }
        Ok(Self {
            n_max,
            t_cooldown,
            t_timeout,
            n: 0,
            t_last: f64::NEG_INFINITY,
            seen: [0; M],
            pending: [None; M],
            escalate: false,
        })
    }

    /// Number of requests accepted since the last authorised reset.
    #[must_use]
    pub fn attempts(&self) -> usize {
        self.n
    }

    /// Escalation flag; it is part of the evidence `S` (Definition 7.4).
    #[must_use]
    pub fn escalated(&self) -> bool {
        self.escalate
    }

    /// Number of requests currently pending.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending[..self.n]
            .iter()
            .filter(|p| p.is_some())
            .count()
    }

    fn index_of(&self, ident: u32) -> Option<usize> {
        self.seen[..self.n].iter().position(|&i| i == ident)
    }

    /// Submits the request `(t, ident, guard)`; the first applicable rule of
    /// Definition 7.13 decides. A non-finite `t` is rejected by the cooldown
    /// rule, so malformed time can never cause an acceptance.
    pub fn request(&mut self, t: f64, ident: u32, guard: bool) -> Outcome {
        if self.index_of(ident).is_some() {
            return Outcome::Duplicate;
        }
        if !guard {
            return Outcome::RejectedGuard;
        }
        if self.n >= self.n_max {
            self.escalate = true;
            return Outcome::RejectedLimit;
        }
        if !(t - self.t_last >= self.t_cooldown) || !t.is_finite() {
            return Outcome::RejectedCooldown;
        }
        self.seen[self.n] = ident;
        self.pending[self.n] = Some(t);
        self.n += 1;
        self.t_last = t;
        Outcome::Accepted
    }

    /// Closes a pending request with its outcome. A failure while the budget
    /// is exhausted raises the escalation flag.
    pub fn resolve(&mut self, ident: u32, success: bool) {
        if let Some(i) = self.index_of(ident) {
            if self.pending[i].take().is_some() && !success && self.n >= self.n_max {
                self.escalate = true;
            }
        }
    }

    /// Closes as failed every pending request older than `t_timeout` and
    /// returns how many were closed.
    pub fn tick(&mut self, t: f64) -> usize {
        let mut expired = 0;
        for i in 0..self.n {
            if let Some(t0) = self.pending[i] {
                if t - t0 >= self.t_timeout {
                    self.pending[i] = None;
                    expired += 1;
                    if self.n >= self.n_max {
                        self.escalate = true;
                    }
                }
            }
        }
        expired
    }

    /// Authorised reset: clears the counter, the seen identifiers, the
    /// pending table and the escalation flag. The time of the last accepted
    /// request is kept, so the cooldown survives resets.
    pub fn authorised_reset(&mut self) {
        self.n = 0;
        self.pending = [None; M];
        self.escalate = false;
    }
}
