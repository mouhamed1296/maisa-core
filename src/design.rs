//! Closed-form design formulas of the paper (Sections 4, 7 and 9).

/// Theorem 4.6: time for the response to a constant input `p0` to reach
/// `z_c` from `z0`. Returns `0` if `z0 >= z_c` and infinity if
/// `p0 <= alpha z_c` (the input is then never detected).
#[must_use]
pub fn time_to_threshold(alpha: f64, p0: f64, z_c: f64, z0: f64) -> f64 {
    let z_inf = p0 / alpha;
    if z0 >= z_c {
        0.0
    } else if z_inf <= z_c {
        f64::INFINITY
    } else {
        libm::log((z_inf - z0) / (z_inf - z_c)) / alpha
    }
}

/// Theorem 4.7: supremum `M_inf` of the response to a periodic input equal
/// to `p0` for `t_on` then `0` for `t_off`.
#[must_use]
pub fn intermittent_peak_limit(alpha: f64, p0: f64, t_on: f64, t_off: f64) -> f64 {
    let m = p0 / alpha * -libm::expm1(-alpha * t_on);
    m / -libm::expm1(-alpha * (t_on + t_off))
}

/// Theorem 4.7: number of the active phase in which `z_c` is first reached,
/// or infinity if `z_c >= M_inf`.
#[must_use]
pub fn intermittent_cycles_to_threshold(
    alpha: f64,
    p0: f64,
    t_on: f64,
    t_off: f64,
    z_c: f64,
) -> f64 {
    let m_inf = intermittent_peak_limit(alpha, p0, t_on, t_off);
    if m_inf <= z_c {
        return f64::INFINITY;
    }
    let period = t_on + t_off;
    libm::ceil(libm::log(m_inf / (m_inf - z_c)) / (alpha * period) - 1e-12)
}

/// Lemma 7.2: minimum time to climb from `z_exit` to `z_enter` when the
/// input is at most `p_max`; infinity if `p_max / alpha <= z_enter`.
#[must_use]
pub fn rise_time(alpha: f64, p_max: f64, z_exit: f64, z_enter: f64) -> f64 {
    let z_inf = p_max / alpha;
    if z_inf <= z_enter {
        f64::INFINITY
    } else {
        libm::log((z_inf - z_exit) / (z_inf - z_enter)) / alpha
    }
}

/// Theorem 9.1: a pair `(alpha, z_c)` separating the tolerated transient
/// from the required detection exists if and only if this returns true.
#[must_use]
pub fn separation_feasible(u_tol: f64, t_tol: f64, u_det: f64, t_det: f64) -> bool {
    u_det > u_tol || u_det * t_det > u_tol * t_tol
}

/// Theorem 9.1 (c): the admissible interval `(lo, hi]` for `z_c` at rate
/// `alpha`, or `None` if it is empty.
#[must_use]
pub fn threshold_interval(
    alpha: f64,
    u_tol: f64,
    t_tol: f64,
    u_det: f64,
    t_det: f64,
) -> Option<(f64, f64)> {
    let lo = u_tol / alpha * -libm::expm1(-alpha * t_tol);
    let hi = u_det / alpha * -libm::expm1(-alpha * t_det);
    if lo < hi {
        Some((lo, hi))
    } else {
        None
    }
}

/// Theorem 5.7 and Proposition 5.8: guard band `E = rho / (1 - exp(-alpha dt_min))`
/// with `rho = eta * max(z_max, p_max / alpha)` and
/// `eta = (1 + eps_c)(1 + u)^2 - 1`, `eps_c = (5 + alpha dt_max) u`, for
/// binary64 (`u = 2^-53`). Computed to first order in `u`, rounded up by 1 %.
#[must_use]
pub fn guard_band_f64(alpha: f64, p_max: f64, z_max: f64, dt_min: f64, dt_max: f64) -> f64 {
    let u = 1.110_223_024_625_156_5e-16; // 2^-53
    let eta = (7.0 + alpha * dt_max) * u * 1.01;
    let bound = if z_max.is_finite() && z_max > p_max / alpha {
        z_max
    } else {
        p_max / alpha
    };
    eta * bound / -libm::expm1(-alpha * dt_min)
}
