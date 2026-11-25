#!/usr/bin/env python3
"""Compare sars build-exe native stdout against sci's native build-exe reference.

For every rosetta demo: build+run with sars; when that succeeds, build+run with
sci's zig-out/bin/sa. Records MATCH / MISMATCH / SKIP (reference unavailable)
with escaped stdout for audit.
"""
import os
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SARS = os.path.join(ROOT, "target", "release", "sars")
SCI = os.path.normpath(os.path.join(ROOT, "..", "sci", "zig-out", "bin", "sa"))
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
OUT = "/tmp/sars_build_exe_compare"
ENV = dict(os.environ)
ENV["PATH"] = ENV.get("PATH", "") + ":/tools/zig"
ENV["ZIG"] = "/tools/zig/zig"


def esc(b):
    return b.decode("utf-8", "replace").replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")


def build_run(bin_path, src, exe, extra=None):
    cmd = [bin_path, "build-exe", src, "-o", exe]
    if extra:
        cmd += extra
    b = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90, env=ENV)
    if b.returncode != 0:
        return None
    r = subprocess.run([exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20)
    return (r.returncode, esc(r.stdout), esc(r.stderr))


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sorted(
        d for d in os.listdir(DEMOS)
        if os.path.isfile(os.path.join(DEMOS, d, "main.sa"))
    )
    counts = {"MATCH": 0, "MISMATCH": 0, "REF_SKIP": 0, "SARS_SKIP": 0}
    for d in names:
        src = os.path.join(DEMOS, d, "main.sa")
        ours = build_run(SARS, src, os.path.join(OUT, "ours_" + d + ".out"))
        if ours is None:
            counts["SARS_SKIP"] += 1
            continue
        ref = build_run(SCI, src, os.path.join(OUT, "ref_" + d + ".out"))
        if ref is None:
            counts["REF_SKIP"] += 1
            print("REF_SKIP %s ours=%s" % (d, ours))
            continue
        verdict = "MATCH" if ours == ref else "MISMATCH"
        counts[verdict] += 1
        if verdict == "MISMATCH":
            print("MISMATCH %s ours=%s ref=%s" % (d, ours, ref))
    print("SUMMARY %s" % counts)


if __name__ == "__main__":
    main()
