#!/usr/bin/env python3
"""C backend corpus/env cluster evidence.

Usage: python3 c_corpus_env_harness.py <before_binary> <after_binary> <out.log>

Runs every sci rosetta demo through `build-exe` with both binaries and, on
success, runs the produced native exe. The before binary MUST be the parent
commit's release binary (re-run, not a cached log column). The log records one
line per name:

    name before_rc=<rc> before_stdout='<escaped>' after_build_rc=<rc> run_rc=<rc> stdout='<escaped>'

Summary accounts the 29-name corpus/env target set explicitly, plus any
positive overflow (names outside the target set that also flipped green).
"""
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
TMP = "/tmp/opencode/c_corpus_env"

TARGETS = set("""
115_opaque_pointers 187_opengl_context_swap 247_contract_semver_major_break
251_contract_resource_ownership 253_contract_callback_registration
255_contract_memory_allocator_swap 257_contract_log_facade
460_vtable_mem_spill 468_callback_slot_write 530_vtable_codeval
538_vtable_pair_code 548_vtable_store_code 608_vtable_neg_code
658_vtable_load_code 343_bitcast_i32_u32 344_bitcast_u64_i64
400_sitofp_f32_bits 402_bitcast_i32_f32 403_bitcast_f32_i32
496_npm_os_totalmem 497_npm_os_parallelism 498_npm_path_isabs
499_npm_util_isequal 500_npm_ns_isabs 501_npm_ns_isequal
965_npm_handle_ctor_method 966_npm_handle_string_out
967_npm_handle_bytes_view 968_npm_handle_callback
""".split())
OVERFLOW = set("333_weak_cyclic_reference 335_ts_slice_inverted_bounds".split())


def esc(b):
    return (
        b.decode("utf-8", "replace")
        .replace("\\", "\\\\")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
    )


def run_one(binary, src, exe):
    b = subprocess.run(
        [binary, "build-exe", src, "-o", exe],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=90,
    )
    if b.returncode != 0:
        return b.returncode, None, ""
    try:
        r = subprocess.run(
            [exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20
        )
        return 0, r.returncode, esc(r.stdout)
    except subprocess.TimeoutExpired:
        return 0, 124, "<timeout>"


def one(name, before, after):
    src = os.path.join(DEMOS, name, "main.sa")
    bexe = os.path.join(TMP, "before_" + name + ".out")
    aexe = os.path.join(TMP, "after_" + name + ".out")
    brc, _, bout = run_one(before, src, bexe)
    arc, rrc, out = run_one(after, src, aexe)
    return "%s before_rc=%d before_stdout='%s' after_build_rc=%d run_rc=%s stdout='%s'\n" % (
        name, brc, bout[:120], arc, "-" if rrc is None else rrc, out[:120],
    )


def field(line, key):
    for part in line.split():
        if part.startswith(key):
            return part[len(key):].rstrip("'")
    return ""


def rc0(line, key):
    try:
        return int(field(line, key)) == 0
    except ValueError:
        return False


def after_pass(line):
    return rc0(line, "after_build_rc=") and field(line, "run_rc=") == "0"


def main():
    before, after, outpath = sys.argv[1:4]
    os.makedirs(TMP, exist_ok=True)
    names = sorted(
        n for n in os.listdir(DEMOS) if os.path.isfile(os.path.join(DEMOS, n, "main.sa"))
    )
    t0 = time.time()
    lines = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        for line in pool.map(lambda n: one(n, before, after), names):
            lines.append(line)
    before_rc0 = sum(rc0(l, "before_rc=") for l in lines)
    after_rc0 = sum(after_pass(l) for l in lines)
    regressions = [l for l in lines if rc0(l, "before_rc=") and not after_pass(l)]
    stdout_regressions = [
        l for l in lines
        if rc0(l, "before_rc=") and after_pass(l)
        and field(l, "before_stdout='") != field(l, "stdout='")
    ]
    target_lines = [l for l in lines if l.split()[0] in TARGETS]
    overflow_lines = [l for l in lines if l.split()[0] in OVERFLOW]
    summary = (
        "total=%d\nbefore_rc0=%d\nafter_rc0=%d\n"
        "pass_to_fail=%d\nstdout_regressions=%d\n"
        "target_total=%d\ntarget_pass_before=%d\ntarget_pass_after=%d\n"
        % (
            len(lines), before_rc0, after_rc0,
            len(regressions), len(stdout_regressions),
            len(target_lines),
            sum(rc0(l, "before_rc=") for l in target_lines),
            sum(after_pass(l) for l in target_lines),
        )
    )
    summary += "overflow_total=%d\noverflow_pass_after=%d\n" % (
        len(overflow_lines), sum(after_pass(l) for l in overflow_lines)
    )
    with open(outpath, "w") as f:
        f.write(summary)
        f.writelines(lines)
    print(summary.strip() + " wrote %s in %.1fs" % (outpath, time.time() - t0))


if __name__ == "__main__":
    main()
