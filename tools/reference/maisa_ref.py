"""
MA-ISA reference model (executable specification).

This module is the normative reference for the mathematics in
"MA-ISA: A Mathematical Model of Multi-Axis Integrated State Accumulation
with Complete Proofs". Section and result numbers in comments refer to
that paper. It favours clarity over speed and uses only the standard
library, so it can serve as the oracle for an embedded or Rust port.
"""
from __future__ import annotations

import math
from dataclasses import dataclass, field
from enum import Enum
from typing import Callable, Dict, List, Optional, Sequence

# --------------------------------------------------------------------------
# Section 3: deviation and response
# --------------------------------------------------------------------------


def deviation(y: float, lo: float, hi: float, s: float) -> float:
    """Normalised distance of y to [lo, hi] (Definition 3.1)."""
    if y < lo:
        return (lo - y) / s
    if y > hi:
        return (y - hi) / s
    return 0.0


def response(d: float, deadband: float, power: float = 1.0) -> float:
    """phi(d) = max(0, d - deadband)^power, a member of the class Phi (Def. 3.3)."""
    e = d - deadband
    return e ** power if e > 0.0 else 0.0


# --------------------------------------------------------------------------
# Section 5: gated exact accumulator
# --------------------------------------------------------------------------


@dataclass
class Accumulator:
    """Scalar accumulator z' = -alpha z + p with exact zero-order-hold step.

    step() implements equation (6) of Definition 5.1: hold on invalid ticks, exact update on
    valid ticks, optional saturation at z_max (Proposition 5.10).
    """

    alpha: float
    z: float = 0.0
    z_max: float = math.inf
    saturated: bool = False  # sticky flag: the cap was reached at least once

    def __post_init__(self) -> None:
        if not (self.alpha > 0.0 and math.isfinite(self.alpha)):
            raise ValueError("alpha must be finite and > 0")
        if not (self.z >= 0.0):
            raise ValueError("initial state must be >= 0")

    def step(self, p: float, dt: float, valid: bool = True) -> float:
        if not valid:
            return self.z  # hold: evidence is neither added nor erased
        if not (p >= 0.0 and math.isfinite(p)):
            raise ValueError("input must be finite and >= 0 on valid ticks")
        if not (dt > 0.0 and math.isfinite(dt)):
            raise ValueError("dt must be finite and > 0 on valid ticks")
        x = -self.alpha * dt
        a = math.exp(x)
        b = -math.expm1(x) / self.alpha  # (1 - a)/alpha without cancellation
        z_new = a * self.z + b * p
        if z_new >= self.z_max:
            z_new = self.z_max
            self.saturated = True
        self.z = z_new
        return self.z


# --------------------------------------------------------------------------
# Section 7.1: hysteretic warning flag (Definition 7.1)
# --------------------------------------------------------------------------


@dataclass
class Hysteresis:
    z_enter: float
    z_exit: float
    t_stable: float
    w: bool = False
    below_since: Optional[float] = None  # start time of current below-run

    def __post_init__(self) -> None:
        if not (0.0 <= self.z_exit < self.z_enter):
            raise ValueError("need 0 <= z_exit < z_enter")
        if not (self.t_stable > 0.0):
            raise ValueError("need t_stable > 0")

    def step(self, z: float, t: float, valid: bool = True) -> bool:
        if not valid:
            self.below_since = None  # stability must be shown on valid data
            return self.w
        if z >= self.z_enter:
            self.w = True
            self.below_since = None
        elif z <= self.z_exit:
            if self.below_since is None:
                self.below_since = t
            if t - self.below_since >= self.t_stable:
                self.w = False
        else:
            self.below_since = None
        return self.w


# --------------------------------------------------------------------------
# Section 7.2: critical latch and supervisory automaton
# --------------------------------------------------------------------------


class Q(Enum):
    UNKNOWN = 0
    NORMAL = 1
    DEGRADED = 2
    CORRECTIVE = 3
    SAFE_STATE_REQUESTED = 4
    SAFE_STATE_CONFIRMED = 5
    LOCKED_RECOVERY = 6


INTERVENTION = frozenset(
    {Q.SAFE_STATE_REQUESTED, Q.SAFE_STATE_CONFIRMED, Q.LOCKED_RECOVERY}
)


@dataclass(frozen=True)
class Evidence:
    """Boolean evidence of one evaluation tick (Definition 7.4)."""

    H: bool  # hard-constraint violation
    C: bool  # some accumulator at or above its critical threshold
    S: bool  # safety-critical data loss, or intervention escalation
    V: bool  # all required measurements valid and fresh
    W: bool  # hysteretic warning (OR over accumulators)
    X: bool  # corrective action authorised
    K: bool  # safe state independently confirmed
    R: bool  # recovery independently verified
    A: bool  # authorised reset received

    @property
    def F(self) -> bool:
        return self.H or self.C or self.S


def supervisor_step(q_prev: Q, l_prev: bool, e: Evidence) -> tuple[Q, bool]:
    """One evaluation of the latch (equations 8 and 9) and of Table 1 (Definition 7.5)."""
    release = (
        q_prev is Q.LOCKED_RECOVERY and (not e.F) and e.V and e.R and e.A
    )
    latch = e.F or (l_prev and not release)

    if latch:
        if q_prev not in INTERVENTION:
            q = Q.SAFE_STATE_REQUESTED
        elif q_prev is Q.SAFE_STATE_REQUESTED:
            q = Q.SAFE_STATE_CONFIRMED if e.K else Q.SAFE_STATE_REQUESTED
        elif q_prev is Q.SAFE_STATE_CONFIRMED:
            if not e.K:
                q = Q.SAFE_STATE_REQUESTED
            elif e.F:
                q = Q.SAFE_STATE_CONFIRMED
            else:
                q = Q.LOCKED_RECOVERY
        else:  # LOCKED_RECOVERY
            q = Q.SAFE_STATE_REQUESTED if e.F else Q.LOCKED_RECOVERY
    else:
        if not e.V:
            q = Q.UNKNOWN
        elif e.W:
            q = Q.CORRECTIVE if e.X else Q.DEGRADED
        else:
            q = Q.NORMAL
    return q, latch


def normal_predicate(e: Evidence, latch: bool) -> bool:
    return e.V and not e.H and not e.C and not e.S and not e.W and not latch


# --------------------------------------------------------------------------
# Section 7.3: bounded intervention engine (Definition 7.13)
# --------------------------------------------------------------------------


class Outcome(Enum):
    ACCEPTED = 0
    DUPLICATE = 1
    REJECTED_GUARD = 2
    REJECTED_LIMIT = 3
    REJECTED_COOLDOWN = 4


@dataclass
class InterventionEngine:
    n_max: int
    t_cooldown: float
    t_timeout: float
    n: int = 0
    t_last: float = -math.inf
    seen: set = field(default_factory=set)
    pending: Dict[object, float] = field(default_factory=dict)
    escalate: bool = False
    log: List[tuple] = field(default_factory=list)  # accepted (t, id)

    def request(self, t: float, ident: object, guard: bool) -> Outcome:
        if ident in self.seen:
            return Outcome.DUPLICATE
        if not guard:
            return Outcome.REJECTED_GUARD
        if self.n >= self.n_max:
            self.escalate = True
            return Outcome.REJECTED_LIMIT
        if t - self.t_last < self.t_cooldown:
            return Outcome.REJECTED_COOLDOWN
        self.n += 1
        self.t_last = t
        self.seen.add(ident)
        self.pending[ident] = t
        self.log.append((t, ident))
        return Outcome.ACCEPTED

    def resolve(self, ident: object, success: bool) -> None:
        if ident in self.pending:
            del self.pending[ident]
            if not success and self.n >= self.n_max:
                self.escalate = True

    def tick(self, t: float) -> List[object]:
        """Expire requests older than t_timeout; returns the expired ids."""
        expired = [i for i, t0 in self.pending.items() if t - t0 >= self.t_timeout]
        for i in expired:
            self.resolve(i, success=False)
        return expired

    def authorised_reset(self) -> None:
        self.n = 0
        self.seen.clear()
        self.pending.clear()
        self.escalate = False
        # t_last is deliberately kept: the cooldown survives resets.


# --------------------------------------------------------------------------
# Section 8: the assembled monitor
# --------------------------------------------------------------------------


@dataclass
class Channel:
    """One accumulator channel (axis or relationship) with its thresholds."""

    name: str
    acc: Accumulator
    z_crit: float
    hyst: Hysteresis
    guard_band: float = 0.0  # numerical guard band E of Corollary 5.9

    def critical(self) -> bool:
        return self.acc.z >= self.z_crit - self.guard_band or self.acc.saturated


@dataclass
class Monitor:
    channels: List[Channel]
    dt_max: float
    q: Q = Q.UNKNOWN
    latch: bool = True  # conservative default when no stored latch is readable
    t: float = 0.0

    def tick(
        self,
        t: float,
        inputs: Sequence[float],
        valid: Sequence[bool],
        *,
        H: bool = False,
        S: bool = False,
        X: bool = False,
        K: bool = False,
        R: bool = False,
        A: bool = False,
        required: Optional[Sequence[bool]] = None,
    ) -> Q:
        dt = t - self.t
        clock_ok = 0.0 < dt <= self.dt_max
        self.t = t
        req = required if required is not None else [True] * len(self.channels)
        W = False
        C = False
        V = clock_ok
        for ch, p, v, r in zip(self.channels, inputs, valid, req):
            ok = bool(v) and clock_ok
            ch.acc.step(p if ok else 0.0, dt, ok)
            W = ch.hyst.step(ch.acc.z, t, ok) or W
            C = ch.critical() or C
            if r and not ok:
                V = False
        e = Evidence(H=H, C=C, S=S, V=V, W=W, X=X, K=K, R=R, A=A)
        self.q, self.latch = supervisor_step(self.q, self.latch, e)
        return self.q


# --------------------------------------------------------------------------
# Closed-form design formulas (Sections 4 and 9)
# --------------------------------------------------------------------------


def time_to_threshold(alpha: float, p0: float, z_c: float, z0: float = 0.0) -> float:
    """Theorem 4.6: first time the constant-input trajectory reaches z_c."""
    z_inf = p0 / alpha
    if z0 >= z_c:
        return 0.0
    if z_inf <= z_c:
        return math.inf
    return math.log((z_inf - z0) / (z_inf - z_c)) / alpha


def intermittent_peak_limit(alpha: float, p0: float, t_on: float, t_off: float) -> float:
    """Theorem 4.7: supremum of the response to a periodic on/off input."""
    m = p0 / alpha * -math.expm1(-alpha * t_on)
    return m / -math.expm1(-alpha * (t_on + t_off))


def intermittent_cycles_to_threshold(
    alpha: float, p0: float, t_on: float, t_off: float, z_c: float
) -> float:
    m_inf = intermittent_peak_limit(alpha, p0, t_on, t_off)
    if m_inf <= z_c:
        return math.inf
    period = t_on + t_off
    return math.ceil(math.log(m_inf / (m_inf - z_c)) / (alpha * period) - 1e-12)


def rise_time(alpha: float, p_max: float, z_exit: float, z_enter: float) -> float:
    """Lemma 7.2: minimum time to climb from z_exit to z_enter."""
    z_inf = p_max / alpha
    if z_inf <= z_enter:
        return math.inf
    return math.log((z_inf - z_exit) / (z_inf - z_enter)) / alpha


def separation_feasible(u_tol: float, t_tol: float, u_det: float, t_det: float) -> bool:
    """Theorem 9.1: a separating design (alpha, z_c) exists iff this holds."""
    return u_det > u_tol or u_det * t_det > u_tol * t_tol
