"""A1 stage 2 (regstate_only) fail-before / pass-after + hard-condition evidence.

before = pre-stage-2 working tree (six FnState side tables authoritative).
after  = working tree with the collapsed CapabilityMask-keyed carrier behind a
default-off gate. Records: same unit test compiled in both trees, workspace
tests, source blast radius, 968-name `sars check` parity with per-rc
attribution, and default-off byte equality for both `check` emission and
`build-js` emitted JS. Also records new-path-on parity over the same corpus.
"""
import collections
import os
import re
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor

REPO = "/content/oc-box-1-work/sars"
ROSETTA = "/content/sa_all/sci/demos/rosetta"
TMP = "/tmp/opencode/stage2_evidence"
BEFORE = os.path.join(TMP, "before")
AFTER = os.path.join(TMP, "after")
LOG = os.path.join(REPO, "DELIVERY/stage2_fail_before_pass_after.log")
TEST_REL = "crates/sa-verify/tests/stage2_regstate.rs"
CARGO_TEST = ["cargo", "test", "-p", "sa-verify", "--test", "stage2_regstate"]
ENV = dict(os.environ, PATH="/tools/rust-1.75.0/bin:" + os.environ["PATH"])
NAMES = sorted(
    name for name in os.listdir(ROSETTA)
    if os.path.isfile(os.path.join(ROSETTA, name, "main.sa"))
)


def run(cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, env=ENV, capture_output=True, text=True, timeout=1800)


def copy_after(dest):
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest, exist_ok=True)
    tar = subprocess.run(
        ["tar", "--exclude=.git", "--exclude=target", "-cf", "-", "-C", REPO, "."],
        capture_output=True, check=True,
    )
    subprocess.run(["tar", "-xf", "-", "-C", dest], input=tar.stdout, check=True)


def install_test(tree):
    dest = os.path.join(tree, TEST_REL)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    shutil.copyfile(os.path.join(REPO, TEST_REL), dest)


def release_binary(tree):
    env = dict(ENV, CARGO_TARGET_DIR=os.path.join(tree, "target"))
    subprocess.run(["cargo", "build", "--release"], cwd=tree, env=env,
                   capture_output=True, text=True, check=True, timeout=1800)
    return os.path.join(tree, "target", "release", "sars")


def source_files(tree):
    found = {}
    root = os.path.join(tree, "crates")
    for dirpath, _, filenames in os.walk(root):
        for name in filenames:
            if name.endswith(".rs"):
                full = os.path.join(dirpath, name)
                with open(full, "rb") as handle:
                    found[os.path.relpath(full, tree)] = handle.read()
    return found


def test_summary(text):
    match = re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed", text)
    return match.group(0) if match else "no test result line"


def check_one(args):
    binary, name, extra = args
    proc = subprocess.run(
        [binary, "check", os.path.join(ROSETTA, name, "main.sa")] + extra,
        capture_output=True, timeout=60,
    )
    return name, proc.returncode, proc.stdout, proc.stderr


def corpus(binary, extra):
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = pool.map(check_one, [(binary, name, extra) for name in NAMES])
        return {name: (rc, out, err) for name, rc, out, err in results}


def norm_js(data):
    return b"\n".join(sorted(data.splitlines()))


def build_one(args):
    binary, name, out_dir = args
    out = os.path.join(out_dir, name + ".mjs")
    proc = subprocess.run(
        [binary, "build-js", os.path.join(ROSETTA, name, "main.sa"), "-o", out],
        capture_output=True, timeout=60,
    )
    data = None
    if os.path.isfile(out):
        if proc.returncode == 0:
            with open(out, "rb") as handle:
                data = norm_js(handle.read())
        os.remove(out)
    return name, proc.returncode, data


def emitted_js(binary, tag):
    out_dir = os.path.join(TMP, "js_" + tag)
    shutil.rmtree(out_dir, ignore_errors=True)
    os.makedirs(out_dir, exist_ok=True)
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = pool.map(build_one, [(binary, name, out_dir) for name in NAMES])
        return {name: (rc, data) for name, rc, data in results}


def main():
    assert os.path.isfile(os.path.join(BEFORE, "crates/sa-verify/src/runtime_mask.rs"))
    assert not os.path.exists(os.path.join(BEFORE, "crates/sa-verify/src/regstate.rs"))
    copy_after(AFTER)
    install_test(BEFORE)
    install_test(AFTER)

    b = run(CARGO_TEST, BEFORE)
    a = run(CARGO_TEST, AFTER)
    w = run(["cargo", "test", "--workspace"], REPO)

    src_before = source_files(BEFORE)
    src_after = source_files(AFTER)
    added = sorted(set(src_after) - set(src_before))
    changed = sorted(
        path for path in set(src_after) & set(src_before)
        if src_after[path] != src_before[path]
    )
    removed = sorted(set(src_before) - set(src_after))

    bb = release_binary(BEFORE)
    ab = release_binary(AFTER)
    before = corpus(bb, [])
    after = corpus(ab, [])
    on = corpus(ab, ["--regstate"])
    js_before = emitted_js(bb, "before")
    js_after = emitted_js(ab, "after")

    lines = []
    lines.append("A1 STAGE 2 (regstate_only) — Collapse FnState side tables into CapabilityMask keyed carrier")
    lines.append("baseline=pre-stage-2 working tree; gate=default-off; new path=--regstate")
    lines.append("unit_test=%s" % TEST_REL)
    lines.append("")
    lines.append("== UNIT TEST ==")
    lines.append("before_rc=%d" % b.returncode)
    lines.append("before_result=%s" % test_summary(b.stdout + b.stderr))
    lines.append("before_error=%s" % next(
        (line.strip() for line in (b.stderr + b.stdout).splitlines()
         if "unresolved import" in line or "could not compile" in line),
        "none",
    ))
    lines.append("after_rc=%d" % a.returncode)
    lines.append("after_result=%s" % test_summary(a.stdout + a.stderr))
    lines.extend("  " + line for line in a.stdout.splitlines()
                 if line.startswith("test stage2_"))
    lines.append("")
    lines.append("== WORKSPACE REGRESSION ==")
    lines.append("workspace_rc=%d" % w.returncode)
    lines.append("workspace_results=%s" % " | ".join(
        m.group(0) for m in re.finditer(r"test result: \w+\. \d+ passed; \d+ failed",
                                        w.stdout + w.stderr)))
    lines.append("")
    lines.append("== SOURCE BLAST RADIUS (crates/**/*.rs) ==")
    lines.append("added=%s" % " ".join(added))
    lines.append("changed=%s" % " ".join(changed))
    lines.append("removed=%s" % " ".join(removed))
    lines.append("")
    lines.append("== 968-NAME `sars check` PARITY + PER-RC ATTRIBUTION (default-off) ==")
    lines.append("total=%d" % len(NAMES))
    lines.append("before_rc_distribution=%s" % sorted(collections.Counter(
        rc for rc, _, _ in before.values()).items()))
    lines.append("after_rc_distribution=%s" % sorted(collections.Counter(
        rc for rc, _, _ in after.values()).items()))
    for rc in sorted({rc for rc, _, _ in before.values()} | {rc for rc, _, _ in after.values()}):
        bnames = sorted(n for n, v in before.items() if v[0] == rc)
        anames = sorted(n for n, v in after.items() if v[0] == rc)
        lines.append("rc=%d before_count=%d after_count=%d" % (rc, len(bnames), len(anames)))
        lines.append("  before_names=%s" % " ".join(bnames))
        lines.append("  after_names=%s" % " ".join(anames))
        lines.append("  rc_bucket_identical=%s" % (bnames == anames))
    green_b = {n for n, v in before.items() if v[0] == 0}
    green_a = {n for n, v in after.items() if v[0] == 0}
    lines.append("before_check_pass=%d" % len(green_b))
    lines.append("after_check_pass=%d" % len(green_a))
    lines.append("pass_to_fail=%d" % len(green_b - green_a))
    lines.append("fail_to_pass=%d" % len(green_a - green_b))
    lines.append("REGRESSIONS: %s" % " ".join(sorted(green_b - green_a)))
    lines.append("RECOVERED: %s" % " ".join(sorted(green_a - green_b)))
    lines.append("")
    lines.append("== DEFAULT-OFF BYTE CHECK (flag off == current HEAD) ==")
    byte_diff = sorted(n for n in NAMES if before[n] != after[n])
    lines.append("check_total=%d" % len(NAMES))
    lines.append("check_byte_identical=%d" % (len(NAMES) - len(byte_diff)))
    lines.append("check_byte_diff=%s" % " ".join(byte_diff))
    js_rc_diff = sorted(n for n in NAMES if js_before[n][0] != js_after[n][0])
    js_data_diff = sorted(n for n in NAMES if js_before[n][1] != js_after[n][1])
    lines.append("buildjs_total=%d" % len(NAMES))
    lines.append("buildjs_emitted_bytes_identical=%d" % (len(NAMES) - len(js_data_diff)))
    lines.append("buildjs_rc_diff=%s" % " ".join(js_rc_diff))
    lines.append("buildjs_emitted_diff=%s" % " ".join(js_data_diff))
    lines.append("")
    lines.append("== NEW PATH ON (--regstate) PARITY ==")
    on_rc_diff = sorted(n for n in NAMES if on[n][0] != after[n][0])
    on_byte_diff = sorted(n for n in NAMES if on[n] != after[n])
    lines.append("on_rc_diff=%s" % " ".join(on_rc_diff))
    lines.append("on_byte_diff=%s" % " ".join(on_byte_diff))
    lines.append("on_rc_distribution=%s" % sorted(collections.Counter(
        rc for rc, _, _ in on.values()).items()))
    lines.append("before_binary=%s" % bb)
    lines.append("after_binary=%s" % ab)
    text = "\n".join(lines) + "\n"
    with open(LOG, "w") as handle:
        handle.write(text)
    print(text)


if __name__ == "__main__":
    main()
