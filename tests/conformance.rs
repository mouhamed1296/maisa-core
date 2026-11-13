//! Conformance with the executable reference model of the paper.
//!
//! The files in `tests/data` were produced by `gen_traces.py` from the Python
//! reference model `maisa_ref.py`. Every record is replayed here. Discrete
//! outputs (flags, states, counters, outcomes) must match exactly; floating
//! point outputs must agree to a relative 1e-12.

mod common;
use common::close;
use maisa_core::design::*;
use maisa_core::*;

fn fl(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("bad float {s}"))
}
fn bo(s: &str) -> bool {
    s == "1"
}

#[test]
fn accumulator_matches_reference() {
    let data = include_str!("data/accumulator.txt");
    let mut acc = None;
    let mut steps = 0usize;
    for (ln, line) in data.lines().enumerate() {
        let p: Vec<&str> = line.split(' ').collect();
        match p[0] {
            "N" => acc = Some(Accumulator::new(fl(p[1]), fl(p[2]), fl(p[3])).unwrap()),
            "S" => {
                let a = acc.as_mut().unwrap();
                let out = a.step(fl(p[1]), fl(p[2]), bo(p[3]));
                assert_eq!(out == StepOutcome::Updated, bo(p[3]), "line {}", ln + 1);
                assert!(
                    close(a.value(), fl(p[4])),
                    "line {}: {} vs {}",
                    ln + 1,
                    a.value(),
                    p[4]
                );
                assert_eq!(a.saturated(), bo(p[5]), "line {}", ln + 1);
                steps += 1;
            }
            _ => panic!("line {}", ln + 1),
        }
    }
    assert_eq!(steps, 48_000);
}

#[test]
fn hysteresis_matches_reference() {
    let data = include_str!("data/hysteresis.txt");
    let mut h = None;
    let mut steps = 0usize;
    for (ln, line) in data.lines().enumerate() {
        let p: Vec<&str> = line.split(' ').collect();
        match p[0] {
            "N" => h = Some(Hysteresis::new(fl(p[1]), fl(p[2]), fl(p[3]), bo(p[4])).unwrap()),
            "S" => {
                let w = h.as_mut().unwrap().step(fl(p[1]), fl(p[2]), bo(p[3]));
                assert_eq!(w, bo(p[4]), "line {}", ln + 1);
                steps += 1;
            }
            _ => panic!("line {}", ln + 1),
        }
    }
    assert_eq!(steps, 60_000);
}

#[test]
fn supervisor_matches_reference_on_the_complete_transition_relation() {
    let data = include_str!("data/supervisor.txt");
    let mut n = 0usize;
    for (ln, line) in data.lines().enumerate() {
        let p: Vec<&str> = line.split(' ').collect();
        let q0 = State::from_u8(p[0].parse().unwrap()).unwrap();
        let bits: Vec<bool> = p[2].chars().map(|c| c == '1').collect();
        let e = Evidence {
            h: bits[0],
            c: bits[1],
            s: bits[2],
            v: bits[3],
            w: bits[4],
            x: bits[5],
            k: bits[6],
            r: bits[7],
            a: bits[8],
        };
        let (q1, l1) = supervisor_step(q0, bo(p[1]), &e);
        assert_eq!(q1 as u8, p[3].parse::<u8>().unwrap(), "line {}", ln + 1);
        assert_eq!(l1, bo(p[4]), "line {}", ln + 1);
        n += 1;
    }
    assert_eq!(n, 7 * 2 * 512);
}

#[test]
fn intervention_engine_matches_reference() {
    let data = include_str!("data/engine.txt");
    let mut eng: Option<InterventionEngine<6>> = None;
    let mut ops = 0usize;
    for (ln, line) in data.lines().enumerate() {
        let p: Vec<&str> = line.split(' ').collect();
        let l = ln + 1;
        if p[0] == "N" {
            eng = Some(InterventionEngine::new(p[1].parse().unwrap(), fl(p[2]), fl(p[3])).unwrap());
            continue;
        }
        let e = eng.as_mut().unwrap();
        let tail: &[&str] = match p[0] {
            "A" => {
                e.authorised_reset();
                &p[1..]
            }
            "R" => {
                e.resolve(p[1].parse().unwrap(), bo(p[2]));
                &p[3..]
            }
            "T" => {
                let expired = e.tick(fl(p[1]));
                assert_eq!(expired, p[2].parse::<usize>().unwrap(), "line {l}");
                &p[3..]
            }
            "Q" => {
                let o = e.request(fl(p[1]), p[2].parse().unwrap(), bo(p[3]));
                assert_eq!(o as u8, p[4].parse::<u8>().unwrap(), "line {l}");
                &p[5..]
            }
            _ => panic!("line {l}"),
        };
        assert_eq!(e.attempts(), tail[0].parse::<usize>().unwrap(), "line {l}");
        assert_eq!(e.escalated(), bo(tail[1]), "line {l}");
        if tail.len() > 2 {
            assert_eq!(e.pending(), tail[2].parse::<usize>().unwrap(), "line {l}");
        }
        ops += 1;
    }
    assert_eq!(ops, 75_000);
}

#[test]
fn monitor_matches_reference() {
    let data = include_str!("data/monitor.txt");
    let mut lines = data.lines().enumerate().peekable();
    let mut ticks = 0usize;
    while let Some((_, header)) = lines.next() {
        let h: Vec<&str> = header.split(' ').collect();
        assert_eq!(h[0], "N");
        assert_eq!(h[1], "3");
        let mut cfg = [ChannelConfig {
            alpha: 1.0,
            z_crit: 1.0,
            z_enter: 1.0,
            z_exit: 0.0,
            t_stable: 1.0,
            z_max: f64::INFINITY,
            guard_band: 0.0,
            required: true,
            z0: 0.0,
            w0: false,
        }; 3];
        for c in cfg.iter_mut() {
            let (_, l) = lines.next().unwrap();
            let p: Vec<&str> = l.split(' ').collect();
            assert_eq!(p[0], "C");
            *c = ChannelConfig {
                alpha: fl(p[1]),
                z_crit: fl(p[2]),
                z_enter: fl(p[3]),
                z_exit: fl(p[4]),
                t_stable: fl(p[5]),
                z_max: fl(p[6]),
                guard_band: fl(p[7]),
                required: bo(p[8]),
                z0: 0.0,
                w0: false,
            };
        }
        let mut mon = Monitor::<3>::new(&cfg, fl(h[2]), bo(h[3]), 0.0).unwrap();
        while let Some((ln, l)) = lines.next_if(|(_, l)| l.starts_with('T')) {
            let p: Vec<&str> = l.split(' ').collect();
            let fb: Vec<bool> = p[2].chars().map(|c| c == '1').collect();
            let ext = External {
                h: fb[0],
                s: fb[1],
                x: fb[2],
                k: fb[3],
                r: fb[4],
                a: fb[5],
            };
            let samples = [
                Sample {
                    p: fl(p[3]),
                    valid: bo(p[4]),
                },
                Sample {
                    p: fl(p[5]),
                    valid: bo(p[6]),
                },
                Sample {
                    p: fl(p[7]),
                    valid: bo(p[8]),
                },
            ];
            let out = mon.tick(fl(p[1]), &samples, ext);
            assert_eq!(
                out.state as u8,
                p[9].parse::<u8>().unwrap(),
                "line {}",
                ln + 1
            );
            assert_eq!(out.latch, bo(p[10]), "line {}", ln + 1);
            for i in 0..3 {
                let z = mon.value(i).unwrap();
                assert!(
                    close(z, fl(p[11 + i])),
                    "line {} channel {i}: {z} vs {}",
                    ln + 1,
                    p[11 + i]
                );
                assert_eq!(
                    mon.warning(i).unwrap(),
                    p[14].as_bytes()[i] == b'1',
                    "line {}",
                    ln + 1
                );
            }
            ticks += 1;
        }
    }
    assert_eq!(ticks, 100_000);
}

#[test]
fn deviation_response_and_design_formulas_match_reference() {
    let data = include_str!("data/design.txt");
    let mut n = 0usize;
    for (ln, line) in data.lines().enumerate() {
        let p: Vec<&str> = line.split(' ').collect();
        let a: Vec<f64> = p[1..p.len() - 1].iter().map(|s| fl(s)).collect();
        let want = p[p.len() - 1];
        let got = match p[0] {
            "dev" => deviation(a[0], a[1], a[2], a[3]),
            "resp" => response(a[0], a[1], a[2]),
            "ttt" => time_to_threshold(a[0], a[1], a[2], a[3]),
            "ipl" => intermittent_peak_limit(a[0], a[1], a[2], a[3]),
            "ict" => {
                // An integer-valued result: one ulp of difference in the
                // logarithm can only matter when its argument sits on an
                // integer boundary, which the generator's margin excludes.
                intermittent_cycles_to_threshold(a[0], a[1], a[2], a[3], a[4])
            }
            "rise" => rise_time(a[0], a[1], a[2], a[3]),
            "sep" => {
                assert_eq!(
                    separation_feasible(a[0], a[1], a[2], a[3]),
                    bo(want),
                    "line {}",
                    ln + 1
                );
                n += 1;
                continue;
            }
            other => panic!("unknown record {other}"),
        };
        assert!(
            close(got, fl(want)),
            "line {} ({}): {got} vs {want}",
            ln + 1,
            p[0]
        );
        n += 1;
    }
    assert_eq!(n, 14_000);
}
