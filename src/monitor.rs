//! Section 8 and Appendix A of the paper: the assembled monitor.

use crate::{supervisor_step, Accumulator, ConfigError, Evidence, Hysteresis, State};

/// Parameters of one accumulator channel (an axis or a relationship).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelConfig {
    /// Decay rate `alpha > 0`.
    pub alpha: f64,
    /// Critical threshold.
    pub z_crit: f64,
    /// Warning entry level, `z_exit < z_enter <= z_crit`.
    pub z_enter: f64,
    /// Warning exit level.
    pub z_exit: f64,
    /// Duration of valid sub-exit observation required to clear the warning.
    pub t_stable: f64,
    /// Cap; `f64::INFINITY` disables it. Reaching the cap is treated as critical.
    pub z_max: f64,
    /// Numerical guard band of Corollary 5.9 (0 for exact-arithmetic semantics).
    pub guard_band: f64,
    /// Whether this channel's validity is required for the evaluation (`V`).
    pub required: bool,
    /// Initial accumulator value.
    pub z0: f64,
    /// Initial warning flag.
    pub w0: bool,
}

/// One channel sample: the abnormality input `p` and its validity.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sample {
    /// Abnormality input, e.g. `w * response(deviation(y, ..), ..)`.
    pub p: f64,
    /// The measurements behind `p` are valid and fresh.
    pub valid: bool,
}

/// Evidence that does not come from the accumulators (Definition 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct External {
    /// Hard-constraint violation.
    pub h: bool,
    /// Safety-critical data loss or intervention escalation.
    pub s: bool,
    /// Corrective action authorised.
    pub x: bool,
    /// Safe state independently confirmed.
    pub k: bool,
    /// Recovery independently verified.
    pub r: bool,
    /// Authorised reset received.
    pub a: bool,
}

/// Result of one tick, suitable for the audit log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickOutput {
    /// Supervisory state after the tick.
    pub state: State,
    /// Latch after the tick; persist it when `latch_changed` is true.
    pub latch: bool,
    /// The latch differs from its value before the tick.
    pub latch_changed: bool,
    /// The state became `SafeRequested` at this tick: submit a safe-state request.
    pub request_safe_state: bool,
    /// The evidence that was evaluated.
    pub evidence: Evidence,
    /// The elapsed time was in `(0, dt_max]`.
    pub clock_ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Channel {
    acc: Accumulator,
    hyst: Hysteresis,
    z_crit: f64,
    guard_band: f64,
    required: bool,
}

/// The assembled monitor for `N` channels: Algorithm 1 of the paper.
///
/// End-to-end guarantees: Theorem 8.1 (no alarm in nominal operation under
/// bounded noise) and Theorem 8.2 (a fault of the stated class is latched
/// within `T* + dt_max`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor<const N: usize> {
    channels: [Channel; N],
    dt_max: f64,
    q: State,
    latch: bool,
    t: f64,
}

impl<const N: usize> Monitor<N> {
    /// Builds a monitor.
    ///
    /// `initial_latch` must be the value read from non-volatile storage, or
    /// `true` if that storage cannot be read (Definition 7.5). `t0` is the
    /// time of construction; the first tick measures its elapsed time from it.
    pub fn new(
        configs: &[ChannelConfig; N],
        dt_max: f64,
        initial_latch: bool,
        t0: f64,
    ) -> Result<Self, ConfigError> {
        if !(dt_max > 0.0 && dt_max.is_finite() && t0.is_finite()) {
            return Err(ConfigError::Duration);
        }
        let mut built: [Option<Channel>; N] = [None; N];
        for (slot, c) in built.iter_mut().zip(configs.iter()) {
            if !(c.z_enter <= c.z_crit && c.z_crit <= c.z_max && c.z_crit.is_finite()) {
                return Err(ConfigError::Thresholds);
            }
            if !(c.guard_band >= 0.0 && c.guard_band.is_finite()) {
                return Err(ConfigError::GuardBand);
            }
            *slot = Some(Channel {
                acc: Accumulator::new(c.alpha, c.z0, c.z_max)?,
                hyst: Hysteresis::new(c.z_enter, c.z_exit, c.t_stable, c.w0)?,
                z_crit: c.z_crit,
                guard_band: c.guard_band,
                required: c.required,
            });
        }
        // Every slot was filled above; the fallback is unreachable and only
        // avoids a panic path.
        let mut channels = [Channel {
            acc: Accumulator::new(1.0, 0.0, f64::INFINITY)?,
            hyst: Hysteresis::new(1.0, 0.0, 1.0, false)?,
            z_crit: 1.0,
            guard_band: 0.0,
            required: false,
        }; N];
        for (dst, src) in channels.iter_mut().zip(built.iter()) {
            if let Some(ch) = src {
                *dst = *ch;
            }
        }
        Ok(Self {
            channels,
            dt_max,
            q: State::Unknown,
            latch: initial_latch,
            t: t0,
        })
    }

    /// Current supervisory state.
    #[must_use]
    pub fn state(&self) -> State {
        self.q
    }

    /// Current latch value.
    #[must_use]
    pub fn latch(&self) -> bool {
        self.latch
    }

    /// Current value of accumulator `i`.
    #[must_use]
    pub fn value(&self, i: usize) -> Option<f64> {
        self.channels.get(i).map(|c| c.acc.value())
    }

    /// Current warning flag of channel `i`.
    #[must_use]
    pub fn warning(&self, i: usize) -> Option<bool> {
        self.channels.get(i).map(|c| c.hyst.flag())
    }

    /// One evaluation tick at time `t` (Algorithm 1, lines 1 to 15).
    ///
    /// A tick whose elapsed time is not in `(0, dt_max]`, or whose time is
    /// not finite, is a clock fault: every accumulator is held and `V` is
    /// false. A non-finite `t` does not overwrite the stored time.
    pub fn tick(&mut self, t: f64, samples: &[Sample; N], ext: External) -> TickOutput {
        let dt = t - self.t;
        let clock_ok = dt > 0.0 && dt <= self.dt_max;
        if t.is_finite() {
            self.t = t;
        }
        let mut c = false;
        let mut w = false;
        let mut v = clock_ok;
        for (ch, smp) in self.channels.iter_mut().zip(samples.iter()) {
            let ok = clock_ok && smp.valid && Accumulator::accepts(smp.p, dt);
            ch.acc.step(smp.p, dt, ok);
            w |= ch.hyst.step(ch.acc.value(), t, ok);
            c |= ch.acc.value() >= ch.z_crit - ch.guard_band || ch.acc.saturated();
            if ch.required && !ok {
                v = false;
            }
        }
        let evidence = Evidence {
            h: ext.h,
            c,
            s: ext.s,
            v,
            w,
            x: ext.x,
            k: ext.k,
            r: ext.r,
            a: ext.a,
        };
        let q_prev = self.q;
        let latch_prev = self.latch;
        let (q, latch) = supervisor_step(q_prev, latch_prev, &evidence);
        self.q = q;
        self.latch = latch;
        TickOutput {
            state: q,
            latch,
            latch_changed: latch != latch_prev,
            request_safe_state: q == State::SafeRequested && q_prev != State::SafeRequested,
            evidence,
            clock_ok,
        }
    }
}
