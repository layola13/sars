import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, "/tmp/opencode")
import wasmtime

ROOT = "/content/sa_all/sci/demos/rosetta"
BEFORE = "/tmp/opencode/tw-before/sars"
AFTER = "/content/oc-box-1-work/target/release/sars"
TMP = "/tmp/opencode/tw_full"
os.makedirs(TMP, exist_ok=True)


def run(path):
    engine = wasmtime.Engine()
    store = wasmtime.Store(engine)
    module = wasmtime.Module.from_file(engine, path)
    linker = wasmtime.Linker(engine)
    for imp in module.imports:
        ty = imp.type
        if not isinstance(ty, wasmtime.FuncType):
            return 2003

        def zero(_ty=ty):
            vals = []
            for r in _ty.results:
                if r == wasmtime.ValType.i32():
                    vals.append(wasmtime.Val.i32(0))
                elif r == wasmtime.ValType.i64():
                    vals.append(wasmtime.Val.i64(0))
                elif r == wasmtime.ValType.f32():
                    vals.append(wasmtime.Val.f32(0))
                elif r == wasmtime.ValType.f64():
                    vals.append(wasmtime.Val.f64(0))
                else:
                    vals.append(wasmtime.Val.i64(0))
            return vals if vals else None

        linker.define(store, imp.module, imp.name, wasmtime.Func(store, ty, lambda *a, _z=zero: _z()))
    try:
        inst = linker.instantiate(store, module)
        exports = inst.exports(store)
        funcs = [k for k in list(exports.keys()) if k != "memory"]
        if not funcs:
            return 2004
        f = exports[funcs[0]]
        try:
            res = f(store)
        except wasmtime.Trap:
            return 2002
        if res is None:
            return 0
        if isinstance(res, (list, tuple)):
            if not res:
                return 0
            res = res[-1]
        value = getattr(res, "value", res)
        try:
            return int(value)
        except Exception:
            return 0
    except wasmtime.Trap:
        return 2002
    except Exception:
        return 2005


def one(name):
    path = os.path.join(ROOT, name, "main.sa")
    out = {}
    for tag, binary in (("before", BEFORE), ("after", AFTER)):
        wasm = os.path.join(TMP, name + "." + tag + ".wasm")
        try:
            b = subprocess.run([binary, "build-wasm", path, "-o", wasm],
                               capture_output=True, text=True, timeout=45)
            if b.returncode != 0 or not os.path.exists(wasm):
                out[tag] = 2001
                continue
            out[tag] = run(wasm)
        except Exception:
            out[tag] = 2006
    return name, out


def main():
    names = sorted(n for n in os.listdir(ROOT)
                   if os.path.isfile(os.path.join(ROOT, n, "main.sa")))
    results = {}
    with ThreadPoolExecutor(max_workers=8) as ex:
        for name, out in ex.map(one, names):
            results[name] = out
    rb, reg = [], []
    for n in names:
        o = results[n]
        if o["before"] != 0 and o["after"] == 0:
            rb.append(n)
        if o["before"] == 0 and o["after"] != 0:
            reg.append(n)
    with open("/tmp/opencode/tw_full_results.json", "w") as f:
        import json
        json.dump(results, f)
    with open("/tmp/opencode/tw_full_log.txt", "w") as f:
        f.write(f"total={len(names)}\n")
        f.write(f"before_rc0={sum(1 for n in names if results[n]['before']==0)}\n")
        f.write(f"after_rc0={sum(1 for n in names if results[n]['after']==0)}\n")
        f.write(f"fail_to_pass={len(rb)}\n")
        f.write("RECOVERED: " + " ".join(rb) + "\n")
        f.write(f"pass_to_fail={len(reg)}\n")
        f.write("REGRESSIONS: " + " ".join(reg) + "\n")
        f.write("== PER NAME ==\n")
        for n in names:
            f.write(f"{n} before={results[n]['before']} after={results[n]['after']}\n")
    print(f"total={len(names)} before_rc0={sum(1 for n in names if results[n]['before']==0)} after_rc0={sum(1 for n in names if results[n]['after']==0)}")
    print(f"fail_to_pass={len(rb)} pass_to_fail={len(reg)}")
    print("REGRESSIONS:", " ".join(reg))
    print("RECOVERED:", " ".join(rb))


if __name__ == "__main__":
    main()
