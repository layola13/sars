"""R2 typed-stub cluster evidence (186, 963, 964).

before = zero-valued generic extern stubs (old runner); after = typed stubs that
implement the extern contracts (out-params, handles, ptr/len pairs, error codes).
Same compiler binary on both sides, so the only changed variable is stub typing.
"""
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor

import atomics_harness as generic
import typed_stub_runner as typed

ROSETTA = "/content/sa_all/sci/demos/rosetta"
SUPPORT = "/content/sa_all/sci/demos/support"
BINARY = "/content/oc-box-1-work/target/release/sars"
TMP = "/tmp/opencode/r2_full"
os.makedirs(TMP, exist_ok=True)

R2_NAMES = {
    "186_sqlite_c_api_binding": b"8\n",
    "963_compress_checksum": b"ok\n",
    "964_multipart_checksum": b"ok\n",
}
FALSE_RC_ONLY = {"301_http_client_saasm", "302_http_server_saasm"}


def source(name):
    if name == "once_probe":
        return os.path.join(SUPPORT, name + ".sa")
    return os.path.join(ROSETTA, name, "main.sa")


def one(name):
    wasm = os.path.join(TMP, name + ".wasm")
    b = subprocess.run([BINARY, "build-wasm", source(name), "-o", wasm],
                       capture_output=True, text=True, timeout=60)
    if b.returncode != 0 or not os.path.exists(wasm):
        return name, {"before": 2001, "after": 2001}
    try:
        before = generic.run(wasm)
    except Exception:
        before = 2005
    try:
        after, out = typed.run(wasm)
    except Exception:
        after = 2005
        out = b""
    return name, {"before": before, "after": after, "stdout": out[:60].decode(errors="replace")}


def main():
    names = sorted(n for n in os.listdir(ROSETTA)
                   if os.path.isfile(os.path.join(ROSETTA, n, "main.sa")))
    names.append("once_probe")
    results = {}
    with ThreadPoolExecutor(max_workers=8) as ex:
        for name, out in ex.map(one, names):
            results[name] = out
    def recovered(n):
        o = results[n]
        if o["before"] == 0 or o["after"] != 0:
            return False
        if n in R2_NAMES:
            return o.get("stdout", "") == R2_NAMES[n].decode()
        if n in FALSE_RC_ONLY:
            return False
        return True

    rb = [n for n in names if recovered(n)]
    reg = [n for n in names if results[n]["before"] == 0 and results[n]["after"] != 0]
    with open("/tmp/opencode/r2_full_log.txt", "w") as f:
        f.write(f"total={len(names)}\n")
        f.write(f"before_rc0={sum(1 for n in names if results[n]['before']==0)}\n")
        f.write(f"after_rc0={sum(1 for n in names if results[n]['after']==0)}\n")
        f.write(f"fail_to_pass={len(rb)}\nRECOVERED: " + " ".join(rb) + "\n")
        f.write(f"pass_to_fail={len(reg)}\nREGRESSIONS: " + " ".join(reg) + "\n")
        f.write("== TARGET STDOUT ==\n")
        for n in sorted(list(R2_NAMES) + sorted(FALSE_RC_ONLY)):
            f.write(f"{n} stdout={results[n].get('stdout','')!r} recovered={recovered(n)}\n")
        f.write("== PER NAME ==\n")
        for n in names:
            f.write(f"{n} before={results[n]['before']} after={results[n]['after']}\n")
    print(f"total={len(names)} before_rc0={sum(1 for n in names if results[n]['before']==0)} after_rc0={sum(1 for n in names if results[n]['after']==0)}")
    print(f"fail_to_pass={len(rb)} pass_to_fail={len(reg)}")
    print("RECOVERED:", " ".join(rb))
    print("REGRESSIONS:", " ".join(reg))
    for n in sorted(list(R2_NAMES) + sorted(FALSE_RC_ONLY)):
        print(n, "stdout", repr(results[n].get("stdout", "")), "recovered", recovered(n))


if __name__ == "__main__":
    main()
