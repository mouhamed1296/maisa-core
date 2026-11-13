//! Section 7.2 of the paper: evidence, latch and supervisory automaton.

/// Supervisory state (Definition 7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum State {
    /// Evidence is insufficient to establish integrity.
    Unknown = 0,
    /// The NORMAL predicate holds (Theorem 7.8).
    Normal = 1,
    /// A warning exists and no corrective action is authorised.
    Degraded = 2,
    /// A warning exists and a corrective action is authorised.
    Corrective = 3,
    /// An intervention has been requested and is not yet confirmed.
    SafeRequested = 4,
    /// Independent evidence confirms the safe state.
    SafeConfirmed = 5,
    /// The fault has cleared; release awaits verified recovery and reset.
    LockedRecovery = 6,
}

impl State {
    /// All seven states, in numeric order.
    pub const ALL: [State; 7] = [
        State::Unknown,
        State::Normal,
        State::Degraded,
        State::Corrective,
        State::SafeRequested,
        State::SafeConfirmed,
        State::LockedRecovery,
    ];

    /// Membership in the set of intervention states.
    #[must_use]
    pub const fn is_intervention(self) -> bool {
        matches!(
            self,
            State::SafeRequested | State::SafeConfirmed | State::LockedRecovery
        )
    }

    /// Inverse of `state as u8`.
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<State> {
        match v {
            0 => Some(State::Unknown),
            1 => Some(State::Normal),
            2 => Some(State::Degraded),
            3 => Some(State::Corrective),
            4 => Some(State::SafeRequested),
            5 => Some(State::SafeConfirmed),
            6 => Some(State::LockedRecovery),
            _ => None,
        }
    }
}

/// Boolean evidence of one tick (Definition 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evidence {
    /// A hard constraint is violated.
    pub h: bool,
    /// Some accumulator is at or above its critical threshold.
    pub c: bool,
    /// A safety-critical measurement is lost, or the intervention engine escalates.
    pub s: bool,
    /// All required measurements are valid and fresh.
    pub v: bool,
    /// The hysteretic warning flag of some accumulator is set.
    pub w: bool,
    /// A corrective action is authorised.
    pub x: bool,
    /// The safe state is confirmed by independent evidence.
    pub k: bool,
    /// Recovery has been verified by independent evidence.
    pub r: bool,
    /// An authorised reset has been received.
    pub a: bool,
}

impl Evidence {
    /// The critical event `F = H or C or S`.
    #[must_use]
    pub const fn critical(&self) -> bool {
        self.h || self.c || self.s
    }
}

/// One evaluation of the release condition (8), the latch (9) and the
/// transition function of Table 1. Returns the new state and the new latch.
///
/// Guarantees (paper, for every reachable situation): the state is an
/// intervention state exactly when the latch is set (Theorem 7.7); the state
/// is `Normal` exactly when [`normal_predicate`] holds (Theorem 7.8); a
/// critical event sets the latch in the same call (Theorem 7.9); the latch
/// falls only from `LockedRecovery` with no critical event, valid data,
/// verified recovery and an authorised reset (Theorem 7.10).
#[must_use]
pub const fn supervisor_step(q_prev: State, latch_prev: bool, e: &Evidence) -> (State, bool) {
    let f = e.critical();
    let release = matches!(q_prev, State::LockedRecovery) && !f && e.v && e.r && e.a;
    let latch = f || (latch_prev && !release);

    let q = if latch {
        match q_prev {
            State::SafeRequested => {
                if e.k {
                    State::SafeConfirmed
                } else {
                    State::SafeRequested
                }
            }
            State::SafeConfirmed => {
                if !e.k {
                    State::SafeRequested
                } else if f {
                    State::SafeConfirmed
                } else {
                    State::LockedRecovery
                }
            }
            State::LockedRecovery => {
                if f {
                    State::SafeRequested
                } else {
                    State::LockedRecovery
                }
            }
            _ => State::SafeRequested,
        }
    } else if !e.v {
        State::Unknown
    } else if e.w {
        if e.x {
            State::Corrective
        } else {
            State::Degraded
        }
    } else {
        State::Normal
    };
    (q, latch)
}

/// The NORMAL predicate of Theorem 7.8.
#[must_use]
pub const fn normal_predicate(e: &Evidence, latch: bool) -> bool {
    e.v && !e.h && !e.c && !e.s && !e.w && !latch
}
