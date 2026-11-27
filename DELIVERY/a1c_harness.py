"""A1-c (fallible/immutable/interior_ptr/owns_children bits) fail-before / pass-after.

before = A1-b snapshot (four-group engine, no derived-bit methods);
after  = working tree with the four derived-bit operations. Same unit test in
both trees; also records workspace-test and 968-name `sars check` parity.
"""
import os
import re
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor

REPO = "/content/oc-box-1-work/sars"
ROSETTA = "/content/sa_all/sci/demos/rosetta"
TMP = "/tmp/opencode/a1c_evidence"
LOG = os.path.join(REPO, "DELIVERY/a1c_fail_before_pass_after.log")
TEST_REL = "crates/sa-verify/tests/a1c_derived_bits.rs"
CARGO_TEST = ["cargo", "test", "-p", "sa-verify", "--test", "a1c_derived_bits"]
NAMES = sorted(
    name for name in os.listdir(ROSETTA)
    if os.path.isfile(os.path.join(ROSETTA, name, "main.sa"))
)
ENV = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])


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


def check_one(args):
    binary, name = args
    proc = subprocess.run(
        [binary, "check", os.path.join(ROSETTA, name, "main.sa")],
        capture_output=True, text=True, timeout=60,
    )
    return name, proc.returncode


def check_corpus(tree):
    binary = release_binary(tree)
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = dict(pool.map(check_one, [(binary, name) for name in NAMES]))
    return binary, {name for name, rc in results.items() if rc == 0}


def test_summary(text):
    match = re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed", text)
    return match.group(0) if match else "no test result line"


def main():
    before_src = os.path.join(TMP, "before/crates/sa-verify/src/capability.rs")
    assert os.path.exists(before_src), "A1-b baseline snapshot missing"
    assert "bind_fallible" not in open(before_src).read(), "baseline contaminated with A1-c"
    before = os.path.join(TMP, "before")
    after = os.path.join(TMP, "after")
    copy_after(after)
    install_test(before)
    install_test(after)

    b = run(CARGO_TEST, before)
    a = run(CARGO_TEST, after)
    w = run(["cargo", "test", "--workspace"], REPO)

    try:
        bb, before_green = check_corpus(before)
        ab, after_green = check_corpus(after)
        parity_ok = True
    except Exception as exc:
        bb = ab = "build failed: %r" % (exc,)
        before_green = after_green = None
        parity_ok = False

    lines = []
    lines.append("A1-c DERIVED BITS — fail-before / pass-after")
    lines.append("baseline=A1-b tree snapshot (four-group engine, no derived bits)")
    lines.append("unit_test=%s" % TEST_REL)
    lines.append("")
    lines.append("== UNIT TEST ==")
    lines.append("before_rc=%d" % b.returncode)
    lines.append("before_result=%s" % test_summary(b.stdout + b.stderr))
    lines.append("before_error=%s" % next(
        (line.strip() for line in (b.stderr + b.stdout).splitlines()
         if "no method named" in line or "not found" in line or "could not compile" in line),
        "none",
    ))
    lines.append("after_rc=%d" % a.returncode)
    lines.append("after_result=%s" % test_summary(a.stdout + a.stderr))
    lines.extend("  " + line for line in a.stdout.splitlines()
                 if line.startswith("test a1c_"))
    lines.append("")
    lines.append("== WORKSPACE REGRESSION ==")
    lines.append("workspace_rc=%d" % w.returncode)
    lines.append("workspace_results=%s" % " | ".join(
        m.group(0) for m in re.finditer(r"test result: \w+\. \d+ passed; \d+ failed",
                                        w.stdout + w.stderr)))
    lines.append("")
    lines.append("== 968-NAME `sars check` PARITY ==")
    if parity_ok:
        lines.append("total=%d" % len(NAMES))
        lines.append("before_check_pass=%d" % len(before_green))
        lines.append("after_check_pass=%d" % len(after_green))
        lines.append("pass_to_fail=%d" % len(before_green - after_green))
        lines.append("fail_to_pass=%d" % len(after_green - before_green))
        lines.append("REGRESSIONS: %s" % " ".join(sorted(before_green - after_green)))
        lines.append("RECOVERED: %s" % " ".join(sorted(after_green - before_green)))
        lines.append("before_binary=%s" % bb)
        lines.append("after_binary=%s" % ab)
    else:
        lines.append("parity_unavailable=%s" % (bb if isinstance(bb, str) else "check failed"))
    text = "\n".join(lines) + "\n"
    with open(LOG, "w") as handle:
        handle.write(text)
    print(text)


if __name__ == "__main__":
    main()
