#!/usr/bin/env python3
"""
Mutation testing for maisa-core.

Each mutation plants one deliberate defect in the source. The test suite is
then run; it must FAIL (the mutant is "killed"). A surviving mutant would mean
that the tests cannot see that defect. Run from the crate root:

    python3 tools/mutation_test.py
"""
import pathlib
import subprocess
import sys

M = [  # (file, original, mutated, what the defect is)
 ("accumulator", "let b = -libm::expm1(x) / self.alpha;", "let b = (1.0 - a) / self.alpha;", "naive 1 - exp() instead of expm1"),
 ("accumulator", "a * self.z + b * p;", "a * self.z + b * p * 1.000001;", "input gain off by 1e-6"),
 ("accumulator", "let x = -self.alpha * dt;", "let x = -self.alpha * dt * 1.000001;", "decay rate off by 1e-6"),
 ("accumulator", "if !valid {\n            return StepOutcome::Held;\n        }", "if !valid {\n            self.z *= 0.999;\n            return StepOutcome::Held;\n        }", "decay during a gated tick"),
 ("accumulator", "if !(z_new < self.z_max) {", "if z_new > self.z_max {", "cap reached exactly is not flagged"),
 ("accumulator", "p >= 0.0 && p.is_finite() && dt > 0.0 && dt.is_finite()", "p.is_finite() && dt > 0.0 && dt.is_finite()", "negative input accepted"),
 ("accumulator", "p >= 0.0 && p.is_finite() && dt > 0.0 && dt.is_finite()", "p >= 0.0 && p.is_finite() && dt >= 0.0 && dt.is_finite()", "zero time step accepted"),
 ("hysteresis", "if z >= self.z_enter {", "if z > self.z_enter {", "warning entry strict"),
 ("hysteresis", "} else if z <= self.z_exit {", "} else if z < self.z_exit {", "warning exit strict"),
 ("hysteresis", "if t - since >= self.t_stable {", "if t - since > self.t_stable {", "stability duration strict"),
 ("hysteresis", "if !valid {\n            self.below_since = None;", "if !valid {", "gated tick does not interrupt the stable run"),
 ("hysteresis", "} else {\n            self.below_since = None;\n        }", "}", "mid-band value does not interrupt the stable run"),
 ("supervisor", "&& !f && e.v && e.r && e.a;", "&& !f && e.v && e.a;", "release without verified recovery"),
 ("supervisor", "&& !f && e.v && e.r && e.a;", "&& !f && e.v && e.r;", "release without authorised reset"),
 ("supervisor", "&& !f && e.v && e.r && e.a;", "&& !f && e.r && e.a;", "release on invalid data"),
 ("supervisor", "matches!(q_prev, State::LockedRecovery) && !f", "!f", "release from any state"),
 ("supervisor", "let latch = f || (latch_prev && !release);", "let latch = f || latch_prev;", "latch can never be released"),
 ("supervisor", "self.h || self.c || self.s", "self.h || self.c", "data-loss flag S ignored"),
 ("supervisor", "self.h || self.c || self.s", "self.c || self.s", "hard constraint ignored"),
 ("supervisor", "_ => State::SafeRequested,", "_ => State::SafeConfirmed,", "confirmation assumed from request"),
 ("supervisor", "if !e.k {\n                    State::SafeRequested\n                } else if f {", "if f {", "lost confirmation not noticed"),
 ("supervisor", "} else if !e.v {\n        State::Unknown", "} else if !e.v {\n        State::Degraded", "invalid data reported as degraded"),
 ("supervisor", "if e.x {\n            State::Corrective\n        } else {\n            State::Degraded\n        }", "if e.x {\n            State::Degraded\n        } else {\n            State::Corrective\n        }", "corrective without authorisation"),
 ("supervisor", "e.v && !e.h && !e.c && !e.s && !e.w && !latch", "e.v && !e.h && !e.c && !e.s && !latch", "NORMAL predicate ignores warnings"),
 ("intervention", "if self.n >= self.n_max {\n            self.escalate = true;\n            return Outcome::RejectedLimit;", "if self.n > self.n_max {\n            self.escalate = true;\n            return Outcome::RejectedLimit;", "one attempt too many"),
 ("intervention", "if self.n >= self.n_max {\n            self.escalate = true;\n            return Outcome::RejectedLimit;", "if self.n >= self.n_max {\n            return Outcome::RejectedLimit;", "exhausted budget not escalated"),
 ("intervention", "if !(t - self.t_last >= self.t_cooldown)", "if !(t - self.t_last > self.t_cooldown)", "cooldown strict"),
 ("intervention", "self.t_last = t;\n", "", "cooldown never armed"),
 ("intervention", "self.n = 0;\n", "self.n = 0;\n        self.t_last = f64::NEG_INFINITY;\n", "reset clears the cooldown"),
 ("intervention", "if t - t0 >= self.t_timeout {", "if t - t0 > self.t_timeout {", "timeout strict"),
 ("intervention", "&& !success && self.n >= self.n_max", "&& success && self.n >= self.n_max", "escalation on success instead of failure"),
 ("intervention", "if !guard {\n            return Outcome::RejectedGuard;\n        }", "", "authorisation guard ignored"),
 ("intervention", "|| !t.is_finite()", "", "non-finite time accepted"),
 ("monitor", "dt > 0.0 && dt <= self.dt_max", "dt > 0.0", "late tick accepted"),
 ("monitor", "dt > 0.0 && dt <= self.dt_max", "dt >= 0.0 && dt <= self.dt_max", "stalled clock accepted"),
 ("monitor", ">= ch.z_crit - ch.guard_band", ">= ch.z_crit", "guard band ignored"),
 ("monitor", "|| ch.acc.saturated()", "", "saturation not treated as critical"),
 ("monitor", "if ch.required && !ok {", "if !ok {", "optional channel treated as required"),
 ("monitor", "if ch.required && !ok {", "if ch.required && !smp.valid {", "malformed or clock-faulted sample counted as valid"),
 ("monitor", "let mut v = clock_ok;", "let mut v = true;", "clock fault does not invalidate"),
 ("monitor", "ch.hyst.step(ch.acc.value(), t, ok)", "ch.hyst.step(ch.acc.value(), t, true)", "hysteresis fed with invalid ticks"),
 ("monitor", "q == State::SafeRequested && q_prev != State::SafeRequested", "q == State::SafeRequested", "request repeated every tick"),
 ("monitor", "if t.is_finite() {\n            self.t = t;\n        }", "self.t = t;", "non-finite time poisons the clock"),
 ("monitor", "w |= ", "w = ", "only the last channel's warning counts"),
 ("monitor", "c |= ", "c = ", "only the last channel's critical flag counts"),
 ("design", "libm::log((z_inf - z0) / (z_inf - z_c)) / alpha\n    }\n}\n\n/// Theorem 4.7: supremum", "libm::log(z_inf / (z_inf - z_c)) / alpha\n    }\n}\n\n/// Theorem 4.7: supremum", "detection time ignores the initial value"),
 ("design", "u_det > u_tol || u_det * t_det > u_tol * t_tol", "u_det > u_tol && u_det * t_det > u_tol * t_tol", "separation criterion with AND"),
 ("design", "libm::log((z_inf - z_exit) / (z_inf - z_enter)) / alpha", "libm::log((z_inf - z_enter) / (z_inf - z_exit)) / alpha", "rise time inverted"),
 ("design", "m / -libm::expm1(-alpha * (t_on + t_off))", "m / -libm::expm1(-alpha * t_off)", "intermittent limit uses the idle time only"),
 ("design", "(7.0 + alpha * dt_max)", "(3.0 + alpha * dt_max)", "guard band too small"),
 ("design", "if lo < hi {", "if lo <= hi {", "empty threshold interval accepted"),
 ("deviation", "(lo - y) / s", "(lo - y) * s", "deviation scaled the wrong way"),
 ("deviation", "(y - hi) / s", "(hi - y) / s", "negative deviation above the set"),
 ("deviation", "} else if y >= lo && y <= hi {\n        0.0\n    } else {\n        f64::NAN\n    }", "} else {\n        0.0\n    }", "NaN observation treated as normal"),
 ("deviation", "let e = d - deadband;", "let e = d;", "deadband ignored"),
]

root = pathlib.Path(__file__).resolve().parent.parent
# Two candidate mutants were examined and left out because they are
# equivalent to the original program, so no test can or should kill them:
#  - latch = (f && !release) || (latch_prev && !release): release implies !f.
#  - time_to_threshold with `z_inf < z_c`: at z_inf == z_c the logarithm of a
#    division by zero already returns infinity.
survivors, errors = [], []
for i, (mod, old, new, what) in enumerate(M, 1):
    path = root / "src" / f"{mod}.rs"
    src = path.read_text()
    if src.count(old) != 1:
        errors.append((i, mod, what, src.count(old)))
        print(f"[SKIP ] {i:2d} {mod}: pattern found {src.count(old)} times: {what}")
        continue
    path.write_text(src.replace(old, new))
    try:
        r = subprocess.run(["cargo", "test", "--release", "-q"], cwd=root, capture_output=True, text=True, timeout=600)
    finally:
        path.write_text(src)
    killed = r.returncode != 0
    print(f"[{'KILLED' if killed else 'ALIVE '}] {i:2d} {mod}: {what}", flush=True)
    if not killed:
        survivors.append((mod, what))
print(f"\n{len(M) - len(survivors) - len(errors)}/{len(M)} mutants killed, {len(survivors)} survived, {len(errors)} not applied")
sys.exit(1 if survivors or errors else 0)
