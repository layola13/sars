"""F cluster evidence: wasm call_indirect heterogeneous signatures (539).

before = snapshot of HEAD 987a066 (first-vtable-signature heuristic).
after  = current tree (resolve the vtable target's own signature per call site).
Same typed-stub runner mode ("c") on both sides; only the compiler changed.
"""
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor

import typed_stub_runner as typed

ROSETTA = "/content/sa_all/sci/demos/rosetta"
SUPPORT = "/content/sa_all/sci/demos/support"
BEFORE = "/tmp/opencode/f_before/sars"
AFTER = "/content/sa_all/sars/target/release/sars"
TMP = "/tmp/opencode/f_full"
os.makedirs(TMP, exist_ok=True)

F_NAMES = {"539_callback_codeval": b"539\n"}


def source(name):
    if name == "once_probe":
        return os.path.join(SUPPORT, name + ".sa")
    return os.path.join(ROSETTA, name, "main.sa")


def one(name):
    out = {}
    for tag, binary in (("before", BEFORE), ("after", AFTER)):
        wasm = os.path.join(TMP, tag + "_" + name + ".wasm")
        b = subprocess.run([binary, "build-wasm", source(name), "-o", wasm],
                           capture_output=True, text=True, timeout=60)
        if b.returncode != 0 or not os.path.exists(wasm):
            out[tag] = 2001
            out["out_" + tag] = ""
            continue
        try:
            rc, stdout = typed.run(wasm, mode="c")
        except Exception:
            rc, stdout = 2005, b""
        out[tag] = rc
        out["out_" + tag] = stdout[:60].decode(errors="replace")
    return name, out


def main():
    names = sorted(n for n in os.listdir(ROSETTA)
                   if os.path.isfile(os.path.join(ROSETTA, n, "main.sa")))
    names.append("once_probe")
    results = {}
    with ThreadPoolExecutor(max_workers=8) as ex:
        for name, out in ex.map(one, names):
            results[name] = out

    def verified(n):
        o = results[n]
        if o["before"] == 0 or o["after"] != 0:
            return False
        expected = F_NAMES.get(n)
        return expected is None or o["out_after"] == expected.decode()

    recovered = [n for n in names if verified(n)]
    regressed = [n for n in names if results[n]["before"] == 0 and results[n]["after"] != 0]
    out_changed = [n for n in names
                   if results[n]["before"] == 0
                   and results[n]["out_before"] != results[n]["out_after"]]
    log = "/content/sa_all/sars/DELIVERY/f_fail_before_pass_after.log"
    with open(log, "w") as f:
        f.write("total=%d\n" % len(names))
        f.write("before_rc0=%d\n" % sum(1 for n in names if results[n]["before"] == 0))
        f.write("after_rc0=%d\n" % sum(1 for n in names if results[n]["after"] == 0))
        f.write("fail_to_pass=%d\nRECOVERED: %s\n" % (len(recovered), " ".join(recovered)))
        f.write("pass_to_fail=%d\nREGRESSIONS: %s\n" % (len(regressed), " ".join(regressed)))
        f.write("stdout_changed_on_pass=%d\nCHANGED: %s\n" % (len(out_changed), " ".join(out_changed)))
        f.write("== TARGET STDOUT ==\n")
        for n in sorted(F_NAMES):
            o = results[n]
            f.write("%s before=%s(%r) after=%s(%r) recovered=%s\n" % (
                n, o["before"], o["out_before"], o["after"], o["out_after"], verified(n)))
        f.write("== PER NAME ==\n")
        for n in names:
            o = results[n]
            f.write("%s before=%s(%r) after=%s(%r)\n" % (
                n, o["before"], o["out_before"], o["after"], o["out_after"]))
    print(open(log).read().split("== PER NAME ==")[0], end="")


if __name__ == "__main__":
    main()
