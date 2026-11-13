//! Exhaustive proof, in Rust and independent of the Python model, of the
//! properties of the supervisory automaton (paper, Proposition 7.6 and
//! Theorems 7.7 to 7.12). The system is finite: all 7 x 2 x 2^9 = 7168
//! combinations of state, latch and evidence are enumerated.

use maisa_core::{normal_predicate, supervisor_step, Evidence, State};
use std::collections::{HashSet, VecDeque};

fn all_evidence() -> Vec<Evidence> {
    (0u16..512)
        .map(|m| Evidence {
            h: m & 1 != 0,
            c: m & 2 != 0,
            s: m & 4 != 0,
            v: m & 8 != 0,
            w: m & 16 != 0,
            x: m & 32 != 0,
            k: m & 64 != 0,
            r: m & 128 != 0,
            a: m & 256 != 0,
        })
        .collect()
}

fn all_states() -> Vec<(State, bool)> {
    State::ALL
        .iter()
        .flat_map(|&q| [(q, false), (q, true)])
        .collect()
}

const GOOD: Evidence = Evidence {
    h: false,
    c: false,
    s: false,
    v: true,
    w: false,
    x: false,
    k: true,
    r: true,
    a: true,
};

#[test]
fn single_step_properties_hold_on_all_7168_inputs() {
    let mut count = 0;
    for (q0, l0) in all_states() {
        for e in all_evidence() {
            let (q1, l1) = supervisor_step(q0, l0, &e);
            count += 1;
            // Theorem 7.7
            assert_eq!(q1.is_intervention(), l1);
            // Theorem 7.8, both directions
            assert_eq!(q1 == State::Normal, normal_predicate(&e, l1));
            // Theorem 7.9
            if e.critical() {
                assert!(l1 && q1.is_intervention());
                if !q0.is_intervention() {
                    assert_eq!(q1, State::SafeRequested);
                }
            }
            // Theorem 7.10
            if l0 && !l1 {
                assert!(q0 == State::LockedRecovery && !e.critical() && e.v && e.r && e.a);
            }
            // Theorem 7.11, for predecessors consistent with Theorem 7.7
            if q0.is_intervention() && l0 && !q1.is_intervention() {
                assert_eq!(q0, State::LockedRecovery);
            }
            if q1 == State::SafeConfirmed && q0 != State::SafeConfirmed {
                assert!(q0 == State::SafeRequested && e.k);
            }
            if q1 == State::LockedRecovery && q0 != State::LockedRecovery {
                assert!(q0 == State::SafeConfirmed && e.k && !e.critical());
            }
            // The transition is a pure function.
            assert_eq!(supervisor_step(q0, l0, &e), (q1, l1));
        }
    }
    assert_eq!(count, 7168);
}

#[test]
fn reachable_states_are_exactly_the_seven_consistent_ones() {
    let ev = all_evidence();
    let mut seen = HashSet::new();
    let mut dq = VecDeque::new();
    for init in [(State::Unknown, false), (State::Unknown, true)] {
        for e in &ev {
            let s = supervisor_step(init.0, init.1, e);
            if seen.insert(s) {
                dq.push_back(s);
            }
        }
    }
    while let Some(s) = dq.pop_front() {
        for e in &ev {
            let n = supervisor_step(s.0, s.1, e);
            if seen.insert(n) {
                dq.push_back(n);
            }
        }
    }
    assert_eq!(seen.len(), 7);
    assert!(seen.iter().all(|(q, l)| q.is_intervention() == *l));
}

#[test]
fn bounded_recovery_under_good_evidence() {
    // Theorem 7.12: four ticks from any admissible situation, three from an
    // intervention state, and NORMAL is then kept.
    let mut consistent: Vec<(State, bool)> = all_states()
        .into_iter()
        .filter(|(q, l)| q.is_intervention() == *l)
        .collect();
    consistent.push((State::Unknown, true));
    for s0 in consistent {
        let mut s = s0;
        let mut steps = 0;
        while s.0 != State::Normal {
            s = supervisor_step(s.0, s.1, &GOOD);
            steps += 1;
            assert!(steps <= 4, "from {s0:?}");
        }
        if s0.0.is_intervention() {
            assert!(steps <= 3, "from {s0:?}");
        }
        assert_eq!(supervisor_step(s.0, s.1, &GOOD).0, State::Normal);
    }
}

#[test]
fn latch_cannot_fall_without_reset_or_verification_or_valid_data() {
    // Theorem 7.10 on whole paths: restrict the evidence and explore.
    type Filter = fn(&Evidence) -> bool;
    let filters: [(&str, Filter); 3] = [
        ("no reset", |e| !e.a),
        ("no recovery verification", |e| !e.r),
        ("invalid data", |e| !e.v),
    ];
    for (name, keep) in filters {
        let ev: Vec<Evidence> = all_evidence().into_iter().filter(keep).collect();
        for s0 in all_states().into_iter().filter(|s| s.1) {
            let mut seen = HashSet::from([s0]);
            let mut dq = VecDeque::from([s0]);
            while let Some(s) = dq.pop_front() {
                for e in &ev {
                    let n = supervisor_step(s.0, s.1, e);
                    assert!(n.1, "latch fell with {name} from {s0:?}");
                    assert_ne!(n.0, State::Normal);
                    if seen.insert(n) {
                        dq.push_back(n);
                    }
                }
            }
        }
    }
}

#[test]
fn state_codes_round_trip() {
    for q in State::ALL {
        assert_eq!(State::from_u8(q as u8), Some(q));
    }
    assert_eq!(State::from_u8(7), None);
}
