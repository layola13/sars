import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, "/tmp/opencode")
import wasmtime

ROOT = "/content/sa_all/sci/demos/rosetta"
BEFORE = "/tmp/opencode/b3-before/target/release/sars"
AFTER = "/content/oc-box-1-work/target/release/sars"
TMP = "/tmp/opencode/b3_wasm_runs"
os.makedirs(TMP, exist_ok=True)

B3 = [50, 176, 180] + list(range(366, 372)) + list(range(373, 376)) + \
     list(range(416, 436)) + list(range(512, 522)) + [526]


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
        if not res:
            return 0
        try:
            return int(res[-1].value)
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
                               capture_output=True, text=True, timeout=30)
            out[tag + "_build"] = b.returncode
            if b.returncode != 0 or not os.path.exists(wasm):
                out[tag] = 2001
                continue
            out[tag] = run(wasm)
        except Exception as e:
            out[tag] = 2006
            out[tag + "_err"] = repr(e)
    return name, out


def main():
    names = sorted(n for n in os.listdir(ROOT)
                   if os.path.isfile(os.path.join(ROOT, n, "main.sa")))
    results = {}
    with ThreadPoolExecutor(max_workers=8) as ex:
        for name, out in ex.map(one, names):
            results[name] = out
    by_prefix = {}
    for n in names:
        try:
            by_prefix.setdefault(int(n.split("_")[0]), []).append(n)
        except ValueError:
            pass
    b3_names = sorted(n for p in B3 for n in by_prefix.get(p, []))
    recovered = [n for n in b3_names if results[n]["before"] != 0 and results[n]["after"] == 0]
    still = [n for n in b3_names if results[n]["after"] != 0]
    became_fail = [n for n in b3_names if results[n]["before"] == 0 and results[n]["after"] != 0]
    collateral = [n for n in names if results[n]["before"] == 0 and results[n]["after"] != 0]
    print("b3 expected:", len(B3), "resolved:", len(b3_names))
    print("b3 before rc!=0:", sum(1 for n in b3_names if results[n]["before"] != 0))
    print("b3 before build-fail(2001):", sum(1 for n in b3_names if results[n]["before"] == 2001))
    print("b3 recovered:", len(recovered))
    print("b3 still fail:", len(still), still)
    print("b3 pass->fail:", len(became_fail))
    print("corpus pass->fail:", len(collateral), collateral)
    print("--- recovered ---")
    print(" ".join(recovered))
    print("--- b3 after fail detail ---")
    for n in still:
        print(n, results[n])
    with open("/tmp/opencode/b3_full_results.txt", "w") as f:
        for n in names:
            o = results[n]
            f.write("%s before=%s after=%s\n" % (n, o["before"], o["after"]))


if __name__ == "__main__":
    main()
