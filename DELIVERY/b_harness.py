"""B cluster evidence: #npm_bind build-wasm gate (10 names).

Acceptance is diagnostic alignment (10700 item 1): sci `sa build-wasm` and
sars `build-wasm` both reject loudly with the same diagnostic class; rc is not
required to flip to green.  before = HEAD a4bb214 snapshot, after = current tree.
"""
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor

import typed_stub_runner as typed

ROSETTA = "/content/sa_all/sci/demos/rosetta"
SUPPORT = "/content/sa_all/sci/demos/support"
BEFORE = "/tmp/opencode/b_before/sars"
AFTER = "/content/sa_all/sars/target/release/sars"
SCI = "/content/sa_all/sci/zig-out/bin/sa"
TMP = "/tmp/opencode/b_full"
os.makedirs(TMP, exist_ok=True)
ENV = dict(os.environ)
ENV["PATH"] = os.path.expanduser("~/.cargo/bin") + ":/tools/node/bin:" + ENV["PATH"]

B_NAMES = [
    "496_npm_os_totalmem", "497_npm_os_parallelism", "498_npm_path_isabs",
    "499_npm_util_isequal", "500_npm_ns_isabs", "501_npm_ns_isequal",
    "965_npm_handle_ctor_method", "966_npm_handle_string_out",
    "967_npm_handle_bytes_view", "968_npm_handle_callback",
]


def source(name):
    if name == "once_probe":
        return os.path.join(SUPPORT, name + ".sa")
    return os.path.join(ROSETTA, name, "main.sa")


def one(name):
    out = {}
    for tag, binary in (("before", BEFORE), ("after", AFTER)):
        wasm = os.path.join(TMP, tag + "_" + name + ".wasm")
        b = subprocess.run([binary, "build-wasm", source(name), "-o", wasm],
                           capture_output=True, text=True, timeout=60, env=ENV)
        if b.returncode != 0 or not os.path.exists(wasm):
            out[tag] = 2001
            out["out_" + tag] = ""
            out["err_" + tag] = " ".join(b.stderr.splitlines()[:1])[:160]
            continue
        try:
            rc, stdout = typed.run(wasm, mode="c")
        except Exception:
            rc, stdout = 2005, b""
        out[tag] = rc
        out["out_" + tag] = stdout[:60].decode(errors="replace")
        out["err_" + tag] = ""
    return name, out


def diagnostics():
    rows = []
    for name in B_NAMES:
        src = source(name)
        sci = subprocess.run([SCI, "build-wasm", src, "-o", os.path.join(TMP, "sci.wasm")],
                             capture_output=True, text=True, timeout=60, env=ENV)
        sars = subprocess.run([AFTER, "build-wasm", src, "-o", os.path.join(TMP, "sars.wasm")],
                              capture_output=True, text=True, timeout=60, env=ENV)
        sci_msg = " ".join((sci.stderr or sci.stdout).splitlines()[:1])[:200]
        sars_msg = " ".join((sars.stderr or sars.stdout).splitlines()[:1])[:200]
        aligned = (sci.returncode != 0 and sars.returncode != 0
                   and "#npm_bind" in sci_msg and "#npm_bind" in sars_msg
                   and "JS" in sci_msg and "JS-only" in sars_msg)
        rows.append((name, sci.returncode, sci_msg, sars.returncode, sars_msg, aligned))
    return rows


def js_paths():
    good = []
    for name in B_NAMES:
        out = os.path.join(TMP, name + ".mjs")
        b = subprocess.run([AFTER, "build-js", source(name), "-o", out],
                           capture_output=True, text=True, timeout=60, env=ENV)
        if b.returncode != 0:
            good.append((name, "build-fail"))
            continue
        r = subprocess.run(["/tools/node/bin/node", out], capture_output=True, timeout=30, env=ENV)
        good.append((name, "rc=%d out=%r" % (r.returncode, r.stdout[:20])))
    return good


def main():
    names = sorted(n for n in os.listdir(ROSETTA)
                   if os.path.isfile(os.path.join(ROSETTA, n, "main.sa")))
    names.append("once_probe")
    results = {}
    with ThreadPoolExecutor(max_workers=8) as ex:
        for name, out in ex.map(one, names):
            results[name] = out
    regressed = [n for n in names if results[n]["before"] == 0 and results[n]["after"] != 0]
    out_changed = [n for n in names
                   if results[n]["before"] == 0
                   and results[n]["out_before"] != results[n]["out_after"]]
    flips = [n for n in B_NAMES if results[n]["before"] != results[n]["after"]]
    diag = diagnostics()
    js = js_paths()
    log = "/content/sa_all/sars/DELIVERY/b_fail_before_pass_after.log"
    with open(log, "w") as f:
        f.write("total=%d\n" % len(names))
        f.write("before_rc0=%d\n" % sum(1 for n in names if results[n]["before"] == 0))
        f.write("after_rc0=%d\n" % sum(1 for n in names if results[n]["after"] == 0))
        f.write("pass_to_fail=%d\nREGRESSIONS: %s\n" % (len(regressed), " ".join(regressed)))
        f.write("stdout_changed_on_pass=%d\nCHANGED: %s\n" % (len(out_changed), " ".join(out_changed)))
        f.write("npm_before_after_rc_changed=%d\nCHANGED_NPM: %s\n" % (len(flips), " ".join(flips)))
        f.write("== DIAGNOSTIC ALIGNMENT ==\n")
        for name, src, sm, ar, am, ok in diag:
            f.write("%s aligned=%s sci_rc=%s sci=%r sars_rc=%s sars=%r\n" % (
                name, ok, src, sm, ar, am))
        f.write("== JS PATH UNAFFECTED ==\n")
        for name, status in js:
            f.write("%s %s\n" % (name, status))
        f.write("== PER NAME ==\n")
        for n in names:
            o = results[n]
            f.write("%s before=%s(%r) after=%s(%r)\n" % (
                n, o["before"], o["out_before"], o["after"], o["out_after"]))
    print(open(log).read().split("== PER NAME ==")[0], end="")


if __name__ == "__main__":
    main()
