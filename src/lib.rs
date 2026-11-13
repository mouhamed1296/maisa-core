//! # maisa-core
//!
//! Implementation of **MA-ISA Mathematical Specification 1.0** ("the paper").
//! Every item states the definition, equation or theorem of the paper that it
//! implements. The crate is `no_std`, performs no allocation, contains no
//! `unsafe` code and never panics on any input value.
//!
//! The normative procedure is Algorithm 1 of the paper, implemented by
//! [`Monitor::tick`]. The building blocks are usable on their own:
//!
//! | Paper | Item |
//! |---|---|
//! | Definition 3.1, 3.3 | [`deviation`], [`response`] |
//! | Definition 5.1, equation (6) | [`Accumulator`] |
//! | Definition 7.1 | [`Hysteresis`] |
//! | Definition 7.4, 7.5, Table 1 | [`Evidence`], [`State`], [`supervisor_step`] |
//! | Definition 7.13 | [`InterventionEngine`] |
//! | Algorithm 1 | [`Monitor`] |
//! | Theorems 4.6, 4.7, 9.1, Lemma 7.2 | [`design`] |
//!
//! All mathematics goes through the pure-Rust `libm` crate, so a desktop
//! build and a microcontroller build produce bit-identical results.
//!
//! ## Deliberate refinement of the reference model
//!
//! The Python reference model raises an exception when a tick is declared
//! valid but carries a negative or non-finite input or a non-positive time
//! step. Firmware must not stop, so this crate treats such a tick as
//! *invalid*: the accumulator is held and the tick does not count as valid
//! evidence. This is the gate of Definition 5.1 applied to malformed data.
//! Likewise, an overflow of the accumulator (possible only with absurd
//! parameters and no finite cap) stores `f64::MAX` and raises the saturation
//! flag, which the monitor treats as critical, instead of storing infinity;
//! and [`deviation`] returns NaN for a NaN observation instead of zero.
//!
//! ## Example
//!
//! ```
//! use maisa_core::{deviation, response, ChannelConfig, External, Monitor, Sample, State};
//!
//! // One pressure axis: acceptable set [5.5, 6.5] bar, scale 0.5 bar,
//! // sensor error 0.05 bar, hence deadband 0.1 (paper, Example 9.2).
//! let cfg = ChannelConfig {
//!     alpha: 1.0, z_crit: 0.6, z_enter: 0.3, z_exit: 0.15, t_stable: 1.0,
//!     z_max: f64::INFINITY, guard_band: 0.0, required: true, z0: 0.0, w0: false,
//! };
//! let mut monitor = Monitor::<1>::new(&[cfg], 0.05, false, 0.0).unwrap();
//!
//! let mut t = 0.0;
//! let mut state = State::Unknown;
//! // A persistent excursion to 7.1 bar: detected at T* = ln 3 = 1.099 s.
//! while t < 1.2 {
//!     t += 0.01;
//!     let p = response(deviation(7.1, 5.5, 6.5, 0.5), 0.1, 1.0);
//!     state = monitor.tick(t, &[Sample { p, valid: true }], External::default()).state;
//! }
//! assert_eq!(state, State::SafeRequested);
//! assert!(monitor.latch());
//! ```
#![no_std]

mod accumulator;
pub mod design;
mod deviation;
mod error;
mod hysteresis;
mod intervention;
mod monitor;
mod supervisor;

pub use accumulator::{Accumulator, StepOutcome};
pub use deviation::{deviation, response};
pub use error::ConfigError;
pub use hysteresis::Hysteresis;
pub use intervention::{InterventionEngine, Outcome};
pub use monitor::{ChannelConfig, External, Monitor, Sample, TickOutput};
pub use supervisor::{normal_predicate, supervisor_step, Evidence, State};
