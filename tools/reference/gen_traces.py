"""
Generates the conformance traces that the Rust crate maisa-core must
reproduce. The Python reference model is the oracle. Output: plain text
files, one record per line, floats written with repr() (round-trip exact).

    python3 gen_traces.py <output-directory>
"""
import itertools
import math
import random
import sys
from pathlib import Path

from maisa_ref import (Accumulator, Channel, Evidence, Hysteresis, InterventionEngine,
                       Monitor, Q, deviation, response, supervisor_step,
                       intermittent_cycles_to_threshold, intermittent_peak_limit,
                       rise_time, separation_feasible, time_to_threshold)

out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
random.seed(1996_01_12)
f = repr
b = lambda x: "1" if x else "0"


def rnd(lo, hi):
    return math.exp(random.uniform(math.log(lo), math.log(hi)))


# ---- accumulator ----------------------------------------------------------
L = []
for case in range(400):
    alpha = rnd(1e-3, 1e3)
    P = rnd(1e-3, 1e3)
    zmax = math.inf if case % 3 else P / alpha * random.uniform(0.2, 1.5)
    z0 = min(random.uniform(0, 2 * P / alpha), zmax) if case % 5 else 0.0
    acc = Accumulator(alpha, z0, zmax)
    L.append(f"N {f(alpha)} {f(z0)} {f(zmax)}")
    for _ in range(120):
        dt = rnd(1e-7, 20) / alpha
        p = random.choice([0.0, P, random.uniform(0, P), rnd(1e-12, 1) * P])
        v = random.random() < 0.9
        acc.step(p, dt, v)
        L.append(f"S {f(p)} {f(dt)} {b(v)} {f(acc.z)} {b(acc.saturated)}")
(out / "accumulator.txt").write_text("\n".join(L) + "\n")

# ---- hysteresis -----------------------------------------------------------
L = []
for case in range(300):
    zen = rnd(0.1, 10); zex = zen * random.uniform(0.0, 0.95); tst = rnd(0.01, 5)
    w0 = random.random() < 0.5
    h = Hysteresis(zen, zex, tst, w=w0)
    L.append(f"N {f(zen)} {f(zex)} {f(tst)} {b(w0)}")
    t = 0.0
    z = random.uniform(0, 2 * zen)
    for _ in range(200):
        t += rnd(1e-3, 1.0) * tst
        r = random.random()
        if r < 0.5:
            z = max(0.0, z + random.uniform(-0.3, 0.3) * zen)
        elif r < 0.6:
            z = random.choice([zen, zex, 0.0, zen * 1.5, (zen + zex) / 2])
        v = random.random() < 0.9
        w = h.step(z, t, v)
        L.append(f"S {f(z)} {f(t)} {b(v)} {b(w)}")
(out / "hysteresis.txt").write_text("\n".join(L) + "\n")

# ---- supervisor: the complete transition relation -------------------------
L = []
for q0 in Q:
    for l0 in (False, True):
        for bits in itertools.product([False, True], repeat=9):
            e = Evidence(*bits)
            q1, l1 = supervisor_step(q0, l0, e)
            L.append(f"{q0.value} {b(l0)} {''.join(b(x) for x in bits)} {q1.value} {b(l1)}")
(out / "supervisor.txt").write_text("\n".join(L) + "\n")

# ---- intervention engine --------------------------------------------------
L = []
for case in range(300):
    nmax = random.randint(1, 6); tcd = rnd(0.1, 5); tto = rnd(0.1, 5)
    eng = InterventionEngine(nmax, tcd, tto)
    L.append(f"N {nmax} {f(tcd)} {f(tto)}")
    t = 0.0
    for _ in range(250):
        t += rnd(1e-3, 2)
        r = random.random()
        if r < 0.03:
            eng.authorised_reset()
            L.append(f"A {eng.n} {b(eng.escalate)}")
        elif r < 0.15:
            ident = random.randint(0, 12); ok = random.random() < 0.5
            eng.resolve(ident, ok)
            L.append(f"R {ident} {b(ok)} {eng.n} {b(eng.escalate)} {len(eng.pending)}")
        elif r < 0.45:
            ex = eng.tick(t)
            L.append(f"T {f(t)} {len(ex)} {eng.n} {b(eng.escalate)} {len(eng.pending)}")
        else:
            ident = random.randint(0, 12); g = random.random() < 0.9
            o = eng.request(t, ident, g)
            L.append(f"Q {f(t)} {ident} {b(g)} {o.value} {eng.n} {b(eng.escalate)} {len(eng.pending)}")
(out / "engine.txt").write_text("\n".join(L) + "\n")

# ---- assembled monitor ----------------------------------------------------
L = []
for case in range(250):
    n = 3
    dtmax = rnd(0.01, 1.0)
    latch0 = random.random() < 0.3
    chans = []; cfg = []
    for i in range(n):
        alpha = rnd(0.05, 20); zc = rnd(0.2, 5)
        zen = zc * random.uniform(0.3, 1.0); zex = zen * random.uniform(0.1, 0.9)
        tst = rnd(0.05, 3); zmax = math.inf if random.random() < 0.7 else zc * random.uniform(1.0, 3.0)
        guard = 0.0 if random.random() < 0.7 else zc * 1e-9
        req = random.random() < 0.8
        chans.append(Channel(f"c{i}", Accumulator(alpha, 0.0, zmax), zc, Hysteresis(zen, zex, tst), guard))
        cfg.append((alpha, zc, zen, zex, tst, zmax, guard, req))
    mon = Monitor(chans, dt_max=dtmax, q=Q.UNKNOWN, latch=latch0, t=0.0)
    L.append(f"N {n} {f(dtmax)} {b(latch0)}")
    for c in cfg:
        L.append("C " + " ".join(f(x) for x in c[:7]) + " " + b(c[7]))
    required = [c[7] for c in cfg]
    t = 0.0
    regime = [0.0] * n
    for k in range(400):
        r = random.random()
        if r < 0.02:
            t += dtmax * random.uniform(1.01, 3.0)      # late tick: clock fault
        elif r < 0.03:
            t -= dtmax * random.uniform(0.0, 0.5)       # time goes backwards or stalls
        else:
            t += dtmax * random.uniform(0.05, 1.0)
        for i in range(n):
            if random.random() < 0.03:
                a_, zc_ = cfg[i][0], cfg[i][1]
                regime[i] = random.choice([0.0, 0.0, 0.5, 1.5, 4.0]) * a_ * zc_
        ps = [regime[i] * random.uniform(0.8, 1.2) for i in range(n)]
        vs = [random.random() < 0.95 for _ in range(n)]
        H = random.random() < 0.004; S = random.random() < 0.004
        X = random.random() < 0.3; K = random.random() < 0.6
        R = random.random() < 0.5; A = random.random() < 0.3
        q = mon.tick(t, ps, vs, H=H, S=S, X=X, K=K, R=R, A=A, required=required)
        L.append("T " + f(t) + " " + "".join(b(x) for x in (H, S, X, K, R, A)) + " "
                 + " ".join(f"{f(p)} {b(v)}" for p, v in zip(ps, vs)) + " "
                 + f"{q.value} {b(mon.latch)} " + " ".join(f(ch.acc.z) for ch in chans)
                 + " " + "".join(b(ch.hyst.w) for ch in chans))
(out / "monitor.txt").write_text("\n".join(L) + "\n")

# ---- deviation, response, design formulas ---------------------------------
L = []
for _ in range(2000):
    lo = random.uniform(-10, 10); hi = lo + random.uniform(0, 10); s = rnd(0.01, 100)
    y = random.choice([lo, hi, random.uniform(lo - 20, hi + 20)])
    L.append(f"dev {f(y)} {f(lo)} {f(hi)} {f(s)} {f(deviation(y, lo, hi, s))}")
    d = random.uniform(0, 5); dead = random.uniform(0, 2); pw = random.choice([1.0, 1.0, 2.0, 1.5, 3.0])
    L.append(f"resp {f(d)} {f(dead)} {f(pw)} {f(response(d, dead, pw))}")
    alpha = rnd(1e-2, 1e2); zc = rnd(1e-2, 1e2); p0 = alpha * zc * random.uniform(0.5, 5); z0 = zc * random.uniform(0, 1.2)
    L.append(f"ttt {f(alpha)} {f(p0)} {f(zc)} {f(z0)} {f(time_to_threshold(alpha, p0, zc, z0))}")
    ton = rnd(1e-2, 2) / alpha; toff = rnd(1e-2, 2) / alpha
    mi = intermittent_peak_limit(alpha, p0, ton, toff)
    zc2 = mi * random.uniform(0.05, 1.5)
    L.append(f"ipl {f(alpha)} {f(p0)} {f(ton)} {f(toff)} {f(mi)}")
    L.append(f"ict {f(alpha)} {f(p0)} {f(ton)} {f(toff)} {f(zc2)} {f(float(intermittent_cycles_to_threshold(alpha, p0, ton, toff, zc2)))}")
    zen = zc; zex = zen * random.uniform(0, 0.99)
    L.append(f"rise {f(alpha)} {f(p0)} {f(zex)} {f(zen)} {f(rise_time(alpha, p0, zex, zen))}")
    a4 = [rnd(0.1, 10) for _ in range(4)]
    L.append("sep " + " ".join(f(x) for x in a4) + " " + b(separation_feasible(*a4)))
(out / "design.txt").write_text("\n".join(L) + "\n")
for p in sorted(out.iterdir()):
    print(f"{p.name}: {sum(1 for _ in p.open())} records")
