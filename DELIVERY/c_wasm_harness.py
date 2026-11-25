"""C(wasm) extern typed-stub cluster evidence (181, 184).

before = R2 typed-stub runner (fd_* / pthread_* still zero-valued generic stubs).
after  = C-mode runner: fd_* contracts and pthread_* contracts, with the real
guest vtable entry invoked through a host-side export hook.
Same compiler binary on both sides; the only changed variable is stub typing.
"""
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor

import typed_stub_runner as typed

ROSETTA = "/content/sa_all/sci/demos/rosetta"
SUPPORT = "/content/sa_all/sci/demos/support"
BINARY = "/content/sa_all/sars/target/release/sars"
TMP = "/tmp/opencode/c_full"
os.makedirs(TMP, exist_ok=True)

C_NAMES = {
    "181_file_descriptor_raii": b"3\n",
    "184_pthread_spawn_join": b"5\n",
}


def source(name):
    if name == "once_probe":
        return os.path.join(SUPPORT, name + ".sa")
    return os.path.join(ROSETTA, name, "main.sa")


def one(name):
    wasm = os.path.join(TMP, name + ".wasm")
    b = subprocess.run([BINARY, "build-wasm", source(name), "-o", wasm],
                       capture_output=True, text=True, timeout=60)
    if b.returncode != 0 or not os.path.exists(wasm):
        return name, {"before": 2001, "after": 2001, "out_before": "", "out_after": ""}
    out = {}
    for tag, mode in (("before", "r2"), ("after", "c")):
        try:
            rc, stdout = typed.run(wasm, mode=mode)
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
        return o["out_after"] == C_NAMES[n].decode() if n in C_NAMES else True

    recovered = [n for n in names if verified(n)]
    regressed = [n for n in names if results[n]["before"] == 0 and results[n]["after"] != 0]
    out_changed = [n for n in names
                   if results[n]["before"] == 0 and results[n]["out_before"] != results[n]["out_after"]]

    with open("/tmp/opencode/c_full_log.txt", "w") as f:
        f.write(f"total={len(names)}\n")
        f.write(f"before_rc0={sum(1 for n in names if results[n]['before']==0)}\n")
        f.write(f"after_rc0={sum(1 for n in names if results[n]['after']==0)}\n")
        f.write(f"fail_to_pass={len(recovered)}\nRECOVERED: " + " ".join(recovered) + "\n")
        f.write(f"pass_to_fail={len(regressed)}\nREGRESSIONS: " + " ".join(regressed) + "\n")
        f.write(f"stdout_changed_on_pass={len(out_changed)}\nCHANGED: " + " ".join(out_changed) + "\n")
        f.write("== TARGET STDOUT ==\n")
        for n in sorted(C_NAMES):
            o = results[n]
            f.write(f"{n} before={o['before']} after={o['after']} "
                    f"out_before={o['out_before']!r} out_after={o['out_after']!r} recovered={verified(n)}\n")
        f.write("== PER NAME ==\n")
        for n in names:
            o = results[n]
            f.write(f"{n} before={o['before']} after={o['after']}\n")

    print(f"total={len(names)} before_rc0={sum(1 for n in names if results[n]['before']==0)} "
          f"after_rc0={sum(1 for n in names if results[n]['after']==0)}")
    print(f"fail_to_pass={len(recovered)} pass_to_fail={len(regressed)} "
          f"stdout_changed_on_pass={len(out_changed)}")
    print("RECOVERED:", " ".join(recovered))
    print("REGRESSIONS:", " ".join(regressed))
    for n in sorted(C_NAMES):
        o = results[n]
        print(n, "before", o["before"], "after", o["after"],
              "out", repr(o["out_after"]), "recovered", verified(n))


if __name__ == "__main__":
    main()
