#!/usr/bin/env python3
"""Writes the example tables the README uses (python3 make_examples.py)."""
import math, random


def write(name, header, rows):
    with open(name, "w") as f:
        f.write("\t".join(header) + "\n")
        for r in rows:
            f.write("\t".join(str(v) for v in r) + "\n")


# Newton's gravity, force = m1*m2/r**2, measured to 6 significant digits
random.seed(7)
rows = []
for _ in range(60):
    m1, m2, r = round(random.uniform(0.5, 5), 3), round(random.uniform(0.5, 5), 3), round(random.uniform(0.5, 3), 3)
    rows.append((m1, m2, r, f"{m1 * m2 / r**2:.6g}"))
write("gravity.tsv", ["m1", "m2", "r", "force"], rows)

# Kepler's third law: the planets and Pluto, both columns to two digits
planets = [(0.387, 0.241), (0.723, 0.615), (1.0, 1.0), (1.524, 1.881), (5.203, 11.86),
           (9.537, 29.46), (19.19, 84.01), (30.07, 164.8), (39.48, 247.9)]
write("planets_2_digits.tsv", ["a_AU", "T_years"], [(f"{a:.2g}", f"{t:.2g}") for a, t in planets])

# the ideal gas law with 1% measurement noise on the pressure
random.seed(11)
rows = []
for T in range(27315, 37316, 500):
    for V in range(10, 31):
        P = 8.314 * (T / 100) / V * (1 + 0.01 * random.gauss(0, 1))
        rows.append((T / 100, V, f"{P:.2f}"))
write("gas_1pct_noise.tsv", ["T_K", "V_L", "P_kPa"], rows)
