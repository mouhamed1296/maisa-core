//! Robustness: malformed data, hostile values and parameter validation.
//! Firmware must not stop, so nothing here may panic, and the invariants of
//! the paper must survive NaN, infinities, negative numbers and a broken
//! clock.

mod common;
use common::Rng;
use maisa_core::*;

#[test]
fn accumulator_holds_and_reports_on_malformed_ticks() {
    let mut a = Accumulator::new(2.0, 1.5, f64::INFINITY).unwrap();
    let before = a.value();
    for (p, dt) in [
        (f64::NAN, 0.1),
        (-1.0, 0.1),
        (f64::INFINITY, 0.1),
        (1.0, 0.0),
        (1.0, -0.1),
        (1.0, f64::NAN),
        (1.0, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
    ] {
        assert_eq!(a.step(p, dt, true), StepOutcome::Rejected, "({p}, {dt})");
        assert_eq!(a.value().to_bits(), before.to_bits());
        assert!(!Accumulator::accepts(p, dt));
    }
    assert_eq!(a.step(f64::NAN, f64::NAN, false), StepOutcome::Held);
    assert_eq!(a.step(1.0, 0.1, true), StepOutcome::Updated);
}

#[test]
fn accumulator_stays_finite_and_non_negative_under_arbitrary_inputs() {
    let mut r = Rng(11);
    for _ in 0..2_000 {
        let alpha = r.log_uniform(1e-300, 1e300);
        let mut a = Accumulator::new(alpha, 0.0, f64::INFINITY).unwrap();
        for _ in 0..200 {
            a.step(r.any_f64(), r.any_f64(), r.chance(0.8));
            assert!(
                a.value().is_finite() && a.value() >= 0.0,
                "value {}",
                a.value()
            );
        }
    }
}

#[test]
fn overflow_is_flagged_as_saturation_not_stored_as_infinity() {
    let mut a = Accumulator::new(1e-300, 0.0, f64::INFINITY).unwrap();
    for _ in 0..10 {
        a.step(f64::MAX, 1e300, true);
    }
    assert!(a.value().is_finite());
    assert!(a.saturated());
}

#[test]
fn monitor_never_panics_and_keeps_its_invariants_under_hostile_input() {
    let mut r = Rng(12);
    for case in 0..1_000 {
        let cfg = ChannelConfig {
            alpha: r.log_uniform(1e-3, 1e3),
            z_crit: 1.0,
            z_enter: 0.5,
            z_exit: 0.2,
            t_stable: 0.5,
            z_max: if case % 2 == 0 { f64::INFINITY } else { 3.0 },
            guard_band: 0.0,
            required: r.chance(0.7),
            z0: 0.0,
            w0: false,
        };
        let mut m = Monitor::<4>::new(&[cfg; 4], 0.1, r.chance(0.5), 0.0).unwrap();
        let mut t = 0.0;
        for _ in 0..400 {
            t = match r.below(10) {
                0 => r.any_f64(),
                1 => t - r.unit(),
                _ => (if t.is_finite() { t } else { 0.0 }) + r.uniform(0.0, 0.15),
            };
            let mut smp = [Sample::default(); 4];
            for s in smp.iter_mut() {
                *s = Sample {
                    p: if r.chance(0.3) {
                        r.any_f64()
                    } else {
                        r.uniform(0.0, 5.0)
                    },
                    valid: r.chance(0.9),
                };
            }
            let ext = External {
                h: r.chance(0.01),
                s: r.chance(0.01),
                x: r.chance(0.5),
                k: r.chance(0.5),
                r: r.chance(0.5),
                a: r.chance(0.5),
            };
            let latch_before = m.latch();
            let state_before = m.state();
            let out = m.tick(t, &smp, ext);
            // Theorem 7.7 and 7.8 in the assembled monitor
            assert_eq!(out.state.is_intervention(), out.latch);
            assert_eq!(
                out.state == State::Normal,
                normal_predicate(&out.evidence, out.latch)
            );
            assert_eq!(out.latch_changed, out.latch != latch_before);
            assert_eq!(
                out.request_safe_state,
                out.state == State::SafeRequested && state_before != State::SafeRequested
            );
            // a malformed required sample or a clock fault can never yield NORMAL
            let malformed_required = cfg.required
                && smp
                    .iter()
                    .any(|s| !s.valid || !(s.p >= 0.0 && s.p.is_finite()));
            if !out.clock_ok || malformed_required {
                assert!(!out.evidence.v);
                assert_ne!(out.state, State::Normal);
            }
            for i in 0..4 {
                let z = m.value(i).unwrap();
                assert!(z.is_finite() && z >= 0.0);
                if z >= cfg.z_enter {
                    assert!(m.warning(i).unwrap());
                }
                if z >= cfg.z_crit {
                    assert!(out.evidence.c && out.latch);
                }
            }
            assert!(m.value(4).is_none() && m.warning(4).is_none());
        }
    }
}

#[test]
fn a_clock_fault_holds_every_accumulator_exactly() {
    let cfg = ChannelConfig {
        alpha: 1.0,
        z_crit: 10.0,
        z_enter: 5.0,
        z_exit: 1.0,
        t_stable: 1.0,
        z_max: f64::INFINITY,
        guard_band: 0.0,
        required: true,
        z0: 0.0,
        w0: false,
    };
    let mut m = Monitor::<1>::new(&[cfg], 0.1, false, 0.0).unwrap();
    let s = [Sample {
        p: 2.0,
        valid: true,
    }];
    m.tick(0.05, &s, External::default());
    let z = m.value(0).unwrap();
    assert!(z > 0.0);
    for bad_t in [0.05, 0.01, 5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let out = m.tick(bad_t, &s, External::default());
        assert!(!out.clock_ok, "t = {bad_t}");
        assert_eq!(out.state, State::Unknown);
        assert_eq!(m.value(0).unwrap().to_bits(), z.to_bits(), "t = {bad_t}");
    }
    // The last finite time seen was 5.0; non-finite times must not have
    // overwritten it, so the clock recovers on the next sane tick.
    let out = m.tick(5.05, &s, External::default());
    assert!(out.clock_ok, "a non-finite time poisoned the clock");
    assert!(m.value(0).unwrap() > z);
}

#[test]
fn boundaries_are_inclusive_exactly_as_specified() {
    // Definition 7.1: the flag clears when t - beta >= T_st (not >).
    let mut h = Hysteresis::new(1.0, 0.5, 2.0, true).unwrap();
    assert!(
        h.step(0.5, 10.0, true),
        "z = z_exit starts the run but does not clear yet"
    );
    assert!(h.step(0.5, 11.0, true));
    assert!(
        !h.step(0.5, 12.0, true),
        "exactly T_st after the run started: cleared"
    );
    // z = z_enter sets the flag (>=).
    assert!(h.step(1.0, 13.0, true));
    // Definition 7.13: a request pending for exactly T_to is closed.
    let mut e = InterventionEngine::<2>::new(2, 1.0, 4.0).unwrap();
    assert_eq!(e.request(1.0, 7, true), Outcome::Accepted);
    assert_eq!(e.tick(4.5), 0);
    assert_eq!(e.tick(5.0), 1, "exactly T_to after acceptance: timed out");
    // ... and a request exactly T_cd after the previous one is accepted.
    assert_eq!(e.request(2.0, 8, true), Outcome::Accepted);
    // Theorem 9.1: an empty interval (lo == hi) is not a design.
    assert_eq!(design::threshold_interval(1.0, 1.0, 1.0, 1.0, 1.0), None);
    assert!(design::threshold_interval(1.0, 1.0, 1.0, 1.0, 1.5).is_some());
    // Theorem 4.6 (b): p0 = alpha * z_c is never detected.
    assert!(design::time_to_threshold(2.0, 2.0, 1.0, 0.0).is_infinite());
    assert_eq!(design::time_to_threshold(2.0, 2.0, 1.0, 1.0), 0.0);
    // The monitor accepts a tick of exactly dt_max and rejects a zero step.
    let cfg = ChannelConfig {
        alpha: 1.0,
        z_crit: 1.0,
        z_enter: 0.5,
        z_exit: 0.2,
        t_stable: 1.0,
        z_max: f64::INFINITY,
        guard_band: 0.0,
        required: true,
        z0: 0.0,
        w0: false,
    };
    let mut m = Monitor::<1>::new(&[cfg], 0.25, false, 0.0).unwrap();
    let s = [Sample {
        p: 0.0,
        valid: true,
    }];
    assert!(m.tick(0.25, &s, External::default()).clock_ok);
    assert!(!m.tick(0.25, &s, External::default()).clock_ok);
    // Critical exactly at the threshold (>=).
    let mut at = cfg;
    at.z0 = 1.0;
    at.w0 = true;
    let mut m = Monitor::<1>::new(&[at], 1.0, false, 0.0).unwrap();
    let hold = [Sample {
        p: 1.0,
        valid: true,
    }]; // equilibrium p/alpha = 1.0 = z_crit
    assert!(m.tick(0.5, &hold, External::default()).evidence.c);
}

#[test]
fn missing_data_holds_evidence_and_reports_unknown() {
    // Definition 5.1: a gated tick neither feeds nor decays the accumulator.
    let cfg = ChannelConfig {
        alpha: 1.0,
        z_crit: 10.0,
        z_enter: 5.0,
        z_exit: 1.0,
        t_stable: 1.0,
        z_max: f64::INFINITY,
        guard_band: 0.0,
        required: true,
        z0: 0.0,
        w0: false,
    };
    let mut m = Monitor::<1>::new(&[cfg], 0.1, false, 0.0).unwrap();
    let mut t = 0.0;
    for _ in 0..20 {
        t += 0.05;
        m.tick(
            t,
            &[Sample {
                p: 2.0,
                valid: true,
            }],
            External::default(),
        );
    }
    let z = m.value(0).unwrap();
    for _ in 0..1000 {
        t += 0.05;
        let out = m.tick(
            t,
            &[Sample {
                p: 0.0,
                valid: false,
            }],
            External::default(),
        );
        assert_eq!(out.state, State::Unknown);
    }
    assert_eq!(
        m.value(0).unwrap().to_bits(),
        z.to_bits(),
        "evidence was erased during the outage"
    );
}

#[test]
fn guard_band_makes_the_critical_test_conservative() {
    let mk = |g: f64| ChannelConfig {
        alpha: 1.0,
        z_crit: 1.0,
        z_enter: 0.5,
        z_exit: 0.2,
        t_stable: 1.0,
        z_max: f64::INFINITY,
        guard_band: g,
        required: true,
        z0: 0.95,
        w0: true,
    };
    let s = [Sample {
        p: 0.95,
        valid: true,
    }]; // equilibrium 0.95 < z_crit
    let mut plain = Monitor::<1>::new(&[mk(0.0)], 1.0, false, 0.0).unwrap();
    let mut banded = Monitor::<1>::new(&[mk(0.1)], 1.0, false, 0.0).unwrap();
    assert!(!plain.tick(0.5, &s, External::default()).evidence.c);
    assert!(banded.tick(0.5, &s, External::default()).evidence.c);
}

#[test]
fn reaching_the_cap_is_critical() {
    let cfg = ChannelConfig {
        alpha: 1.0,
        z_crit: 2.0,
        z_enter: 1.0,
        z_exit: 0.5,
        t_stable: 1.0,
        z_max: 2.0,
        guard_band: 0.0,
        required: true,
        z0: 0.0,
        w0: false,
    };
    let mut m = Monitor::<1>::new(&[cfg], 1.0, false, 0.0).unwrap();
    let mut t = 0.0;
    let mut out = m.tick(
        0.5,
        &[Sample {
            p: 100.0,
            valid: true,
        }],
        External::default(),
    );
    while !out.latch {
        t += 0.5;
        out = m.tick(
            0.5 + t,
            &[Sample {
                p: 100.0,
                valid: true,
            }],
            External::default(),
        );
        assert!(t < 100.0);
    }
    assert_eq!(m.value(0).unwrap(), 2.0);
}

#[test]
fn invalid_parameters_are_rejected() {
    use ConfigError::*;
    let inf = f64::INFINITY;
    assert_eq!(Accumulator::new(0.0, 0.0, inf), Err(Rate));
    assert_eq!(Accumulator::new(-1.0, 0.0, inf), Err(Rate));
    assert_eq!(Accumulator::new(f64::NAN, 0.0, inf), Err(Rate));
    assert_eq!(Accumulator::new(inf, 0.0, inf), Err(Rate));
    assert_eq!(Accumulator::new(1.0, -0.1, inf), Err(InitialValue));
    assert_eq!(Accumulator::new(1.0, f64::NAN, inf), Err(InitialValue));
    assert_eq!(Accumulator::new(1.0, 2.0, 1.0), Err(InitialValue));
    assert_eq!(Accumulator::new(1.0, 0.0, 0.0), Err(Cap));
    assert_eq!(Accumulator::new(1.0, 0.0, f64::NAN), Err(Cap));
    assert!(Accumulator::new(1.0, 1.0, 1.0).is_ok());

    assert_eq!(Hysteresis::new(1.0, 1.0, 1.0, false), Err(Thresholds));
    assert_eq!(Hysteresis::new(1.0, 2.0, 1.0, false), Err(Thresholds));
    assert_eq!(Hysteresis::new(1.0, -0.1, 1.0, false), Err(Thresholds));
    assert_eq!(Hysteresis::new(f64::NAN, 0.5, 1.0, false), Err(Thresholds));
    assert_eq!(Hysteresis::new(inf, 0.5, 1.0, false), Err(Thresholds));
    assert_eq!(Hysteresis::new(1.0, 0.5, 0.0, false), Err(Duration));
    assert_eq!(Hysteresis::new(1.0, 0.5, f64::NAN, false), Err(Duration));

    assert_eq!(InterventionEngine::<3>::new(0, 1.0, 1.0), Err(AttemptLimit));
    assert_eq!(InterventionEngine::<3>::new(4, 1.0, 1.0), Err(AttemptLimit));
    assert_eq!(InterventionEngine::<3>::new(3, 0.0, 1.0), Err(Duration));
    assert_eq!(
        InterventionEngine::<3>::new(3, 1.0, f64::NAN),
        Err(Duration)
    );
    assert!(InterventionEngine::<3>::new(3, 1.0, 1.0).is_ok());

    let ok = ChannelConfig {
        alpha: 1.0,
        z_crit: 1.0,
        z_enter: 0.5,
        z_exit: 0.2,
        t_stable: 1.0,
        z_max: inf,
        guard_band: 0.0,
        required: true,
        z0: 0.0,
        w0: false,
    };
    assert!(Monitor::<2>::new(&[ok; 2], 0.1, true, 0.0).is_ok());
    assert!(Monitor::<0>::new(&[], 0.1, true, 0.0).is_ok());
    let bad = |f: fn(&mut ChannelConfig)| {
        let mut c = ok;
        f(&mut c);
        Monitor::<2>::new(&[ok, c], 0.1, true, 0.0).unwrap_err()
    };
    assert_eq!(bad(|c| c.alpha = 0.0), Rate);
    assert_eq!(bad(|c| c.z_enter = 1.5), Thresholds);
    assert_eq!(bad(|c| c.z_exit = 0.5), Thresholds);
    assert_eq!(bad(|c| c.z_max = 0.9), Thresholds);
    assert_eq!(bad(|c| c.z_crit = f64::NAN), Thresholds);
    assert_eq!(bad(|c| c.t_stable = -1.0), Duration);
    assert_eq!(bad(|c| c.guard_band = -1.0), GuardBand);
    assert_eq!(bad(|c| c.guard_band = f64::NAN), GuardBand);
    assert_eq!(bad(|c| c.z0 = -1.0), InitialValue);
    assert_eq!(
        Monitor::<2>::new(&[ok; 2], 0.0, true, 0.0).unwrap_err(),
        Duration
    );
    assert_eq!(
        Monitor::<2>::new(&[ok; 2], 0.1, true, f64::NAN).unwrap_err(),
        Duration
    );
    // every error has a message
    for e in [
        Rate,
        InitialValue,
        Cap,
        Thresholds,
        Duration,
        GuardBand,
        AttemptLimit,
    ] {
        assert!(!format!("{e}").is_empty());
    }
}

#[test]
fn engine_is_safe_under_hostile_time_and_survives_full_capacity() {
    let mut e = InterventionEngine::<2>::new(2, 1.0, 1.0).unwrap();
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(e.request(t, 1, true), Outcome::RejectedCooldown, "t = {t}");
    }
    assert_eq!(e.request(0.0, 1, true), Outcome::Accepted);
    assert_eq!(e.request(0.5, 2, true), Outcome::RejectedCooldown);
    assert_eq!(e.request(1.0, 1, true), Outcome::Duplicate);
    assert_eq!(e.request(1.0, 2, false), Outcome::RejectedGuard);
    assert_eq!(e.request(1.0, 2, true), Outcome::Accepted);
    assert_eq!(e.pending(), 2);
    assert_eq!(e.tick(f64::NAN), 0);
    assert!(!e.escalated());
    assert_eq!(e.request(5.0, 3, true), Outcome::RejectedLimit);
    assert!(e.escalated());
    assert_eq!(e.tick(5.0), 2);
    assert_eq!(e.pending(), 0);
    e.resolve(99, false); // unknown identifier: no effect
    e.authorised_reset();
    assert!(!e.escalated());
    assert_eq!(
        e.request(5.5, 1, true),
        Outcome::Accepted,
        "identifiers are forgotten by the reset"
    );
    e.authorised_reset();
    assert_eq!(
        e.request(6.0, 2, true),
        Outcome::RejectedCooldown,
        "cooldown survives the reset"
    );
    assert_eq!(e.request(6.5, 2, true), Outcome::Accepted);
    e.resolve(2, true);
    assert_eq!(e.pending(), 0);
    assert!(!e.escalated());
    assert_eq!(e.request(7.5, 3, true), Outcome::Accepted);
    e.resolve(3, false);
    assert!(
        e.escalated(),
        "a failure with the budget exhausted escalates"
    );
}

#[test]
fn deviation_and_response_edge_cases() {
    assert_eq!(deviation(5.0, 4.0, 8.0, 2.0), 0.0);
    assert_eq!(deviation(4.0, 4.0, 8.0, 2.0), 0.0);
    assert_eq!(deviation(8.0, 4.0, 8.0, 2.0), 0.0);
    assert_eq!(deviation(3.0, 4.0, 8.0, 2.0), 0.5);
    assert_eq!(deviation(10.0, 4.0, 8.0, 2.0), 1.0);
    assert!(deviation(f64::NAN, 4.0, 8.0, 2.0).is_nan());
    assert_eq!(deviation(f64::INFINITY, 4.0, 8.0, 2.0), f64::INFINITY);
    assert_eq!(response(0.5, 0.5, 1.0), 0.0);
    assert_eq!(response(0.2, 0.5, 2.0), 0.0);
    assert_eq!(response(1.5, 0.5, 1.0), 1.0);
    assert_eq!(response(2.5, 0.5, 2.0), 4.0);
    assert!(response(f64::NAN, 0.5, 1.0).is_nan());
    // Lemma 3.2 (c): 1/s-Lipschitz
    let mut r = Rng(13);
    for _ in 0..100_000 {
        let (lo, s) = (r.uniform(-10.0, 10.0), r.log_uniform(0.01, 100.0));
        let hi = lo + r.uniform(0.0, 10.0);
        let (y1, y2) = (r.uniform(-40.0, 40.0), r.uniform(-40.0, 40.0));
        let (d1, d2) = (deviation(y1, lo, hi, s), deviation(y2, lo, hi, s));
        assert!(d1 >= 0.0 && (d1 - d2).abs() <= (y1 - y2).abs() / s + 1e-12 * (1.0 + d1 + d2));
        assert_eq!(d1 == 0.0, (lo..=hi).contains(&y1));
    }
}
