# maisa-core

Rust implementation of **MA-ISA Mathematical Specification 1.0**
(Multi-Axis Integrated State Accumulation).

- `no_std`, no allocation, no `unsafe`, one dependency (`libm`, pure Rust).
- Never panics, whatever the input: NaN, infinities, negative values, a clock
  that stalls or runs backwards.
- Same results on a desktop and on a microcontroller, because all mathematics
  goes through `libm`.

## What is in the crate

| Paper | Rust |
|---|---|
| Definitions 3.1, 3.3 | `deviation`, `response` |
| Definition 5.1, equation (6) | `Accumulator` |
| Definition 7.1 | `Hysteresis` |
| Definitions 7.4, 7.5, Table 1 | `Evidence`, `State`, `supervisor_step`, `normal_predicate` |
| Definition 7.13 | `InterventionEngine<M>` |
| Algorithm 1 | `Monitor<N>` and `Monitor::tick` |
| Theorems 4.6, 4.7, 9.1, Lemma 7.2, Corollary 5.9 | module `design` |

## Use

```rust
use maisa_core::{deviation, response, ChannelConfig, External, Monitor, Sample};

let cfg = ChannelConfig {
    alpha: 1.0, z_crit: 0.6, z_enter: 0.3, z_exit: 0.15, t_stable: 1.0,
    z_max: f64::INFINITY, guard_band: 0.0, required: true, z0: 0.0, w0: false,
};
// initial_latch: the value read from non-volatile storage, or true if unreadable
let mut monitor = Monitor::<1>::new(&[cfg], 0.05, false, 0.0).unwrap();

// at every tick:
let p = response(deviation(6.9, 5.5, 6.5, 0.5), 0.1, 1.0);
let out = monitor.tick(0.01, &[Sample { p, valid: true }], External::default());
// out.state, out.latch, out.latch_changed (persist it), out.request_safe_state
```

The platform supplies three things: the time of each tick in seconds,
non-volatile storage for the latch, and the transport of safe-state requests
to the controller (through `InterventionEngine`).

## How it is tested

    ./check.sh          # format, lint, tests (debug and release), docs
    ./check.sh --full   # also mutation testing

| Suite | What it does |
|---|---|
| `tests/conformance.rs` | Replays 304,000 records produced by the Python reference model of the paper. Flags, states, counters and outcomes must match exactly; floating-point values to a relative 1e-12. The supervisor is compared on its complete transition relation (7,168 cases). |
| `tests/supervisor_exhaustive.rs` | Proves in Rust, by enumeration, Theorems 7.7 to 7.12 and the reachable state set. |
| `tests/theorems.rs` | Randomised attempts to falsify Lemma 5.2, Theorems 5.3 to 5.6, 4.7, 7.3, 7.14, 8.1, 8.2, 8.3, 9.1, Proposition 5.10, plus a complete fault and recovery cycle and Example 9.2. |
| `tests/robustness.rs` | Hostile and malformed inputs, clock faults, missing data, overflow, exact boundary cases, parameter validation. |
| `tools/mutation_test.py` | Plants 55 deliberate defects, one at a time, and checks that the tests fail for each. |

The traces in `tests/data` are regenerated with

    python3 tools/reference/gen_traces.py tests/data

## Differences from the Python reference model

Three deliberate refinements, all on the safe side, none reachable with
well-formed data:

1. A tick declared valid with a negative or non-finite input, or a
   non-positive time step, is treated as invalid (the reference raises).
2. An accumulator overflow stores `f64::MAX` and raises the saturation flag,
   which the monitor treats as critical (the reference stores infinity).
3. `deviation` returns NaN for a NaN observation (the reference returns 0).

## Not yet done

- The crate uses `f64`. An `f32` configuration needs its own guard band and
  its own conformance run.
- It has not been cross-compiled for a microcontroller target here. `no_std`
  is enforced by the compiler on the host; building for ESP32 or Cortex-M
  remains to be confirmed on your machine.

## License

MIT. See [LICENSE](LICENSE).
