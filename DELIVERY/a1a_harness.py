"""A1-a (state carrier) fail-before / pass-after evidence.

before = pristine task branch tip (no sa_verify::mask module); after = working tree
with the complete 12-bit u16 carrier and sci TRUTH_TABLE. The exact same unit test
is compiled in both trees, so the only changed variable is the carrier itself.
Also records workspace-test and 968-name `sars check` parity across both trees.
"""
import os
import re
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor

REPO = "/content/oc-box-1-work/sars"
BASELINE = "69a5ff79bc39b557394b8571eee1f89ad96fe7a4"
ROSETTA = "/content/sa_all/sci/demos/rosetta"
TMP = "/tmp/opencode/a1a_evidence"
LOG = os.path.join(REPO, "DELIVERY/a1a_fail_before_pass_after.log")
TEST_REL = "crates/sa-verify/tests/a1a_state_carrier.rs"
CARGO_TEST = ["cargo", "test", "-p", "sa-verify", "--test", "a1a_state_carrier"]
NAMES = sorted(
    name for name in os.listdir(ROSETTA)
    if os.path.isfile(os.path.join(ROSETTA, name, "main.sa"))
)
ENV = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])


def run(cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, env=ENV, capture_output=True, text=True, timeout=1800)


def archive_baseline(dest):
    os.makedirs(dest, exist_ok=True)
    with open(os.path.join(TMP, "baseline.tar"), "wb") as tar:
        subprocess.run(["git", "archive", BASELINE], cwd=REPO, stdout=tar, check=True)
    subprocess.run(["tar", "-xf", os.path.join(TMP, "baseline.tar"), "-C", dest], check=True)


def copy_worktree(dest):
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
    green = {name for name, rc in results.items() if rc == 0}
    return binary, green, results


def test_summary(text):
    match = re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed", text)
    return match.group(0) if match else "no test result line"


def main():
    shutil.rmtree(TMP, ignore_errors=True)
    os.makedirs(TMP, exist_ok=True)
    before = os.path.join(TMP, "before")
    after = os.path.join(TMP, "after")
    archive_baseline(before)
    copy_worktree(after)
    install_test(before)
    install_test(after)

    b = run(CARGO_TEST, before)
    a = run(CARGO_TEST, after)
    w = run(["cargo", "test", "--workspace"], REPO)

    before_green = None
    after_green = None
    before_rcs = after_rcs = None
    try:
        bb, before_green, before_rcs = check_corpus(before)
        ab, after_green, after_rcs = check_corpus(after)
    except Exception as exc:
        bb = ab = "build failed: %r" % (exc,)

    lines = []
    lines.append("A1-a STATE CARRIER — fail-before / pass-after")
    lines.append("baseline_commit=%s" % BASELINE)
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
                 if line.startswith("test a1a_"))
    lines.append("")
    lines.append("== WORKSPACE REGRESSION ==")
    lines.append("workspace_rc=%d" % w.returncode)
    lines.append("workspace_results=%s" % " | ".join(
        m.group(0) for m in re.finditer(r"test result: \w+\. \d+ passed; \d+ failed",
                                        w.stdout + w.stderr)))
    lines.append("")
    lines.append("== 968-NAME `sars check` PARITY ==")
    if before_green is not None and after_green is not None:
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
