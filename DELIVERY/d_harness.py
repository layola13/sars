import json, os, subprocess
from concurrent.futures import ThreadPoolExecutor

ENVP = dict(os.environ)
ENVP["PATH"] = os.path.expanduser("~/.cargo/bin") + ":/tools/node/bin:" + ENVP["PATH"]
BEFORE = "/tmp/opencode/d_before_target/release/sars"
AFTER = "/content/sa_all/sars/target/release/sars"
ROSETTA = "/content/sa_all/sci/demos/rosetta"
SUPPORT = "/content/sa_all/sci/demos/support"
TMP = "/tmp/opencode/d_js_compare"
os.makedirs(TMP, exist_ok=True)

JS_SUPPORT = ["once_probe", "hashmap_probe", "hashset_probe", "sort_probe", "mpsc_probe"]
NAMES = sorted(n for n in os.listdir(ROSETTA) if os.path.isfile(os.path.join(ROSETTA, n, "main.sa"))) + JS_SUPPORT

def source(name):
    sp = os.path.join(SUPPORT, name + ".sa")
    return sp if os.path.isfile(sp) else os.path.join(ROSETTA, name, "main.sa")

def one(args):
    tag, binary, name = args
    out = os.path.join(TMP, "%s_%s.mjs" % (tag, name))
    rec = {"rc": 2001, "stdout": "", "err": ""}
    try:
        b = subprocess.run([binary, "build-js", source(name), "-o", out],
                           capture_output=True, text=True, timeout=60, env=ENVP)
        if b.returncode != 0 or not os.path.exists(out):
            rec["err"] = b.stderr[:200]
            return tag, name, rec
    except Exception as e:
        rec["rc"] = 2006
        rec["err"] = repr(e)
        return tag, name, rec
    try:
        r = subprocess.run(["/tools/node/bin/node", out], capture_output=True, timeout=30, env=ENVP)
        rec["rc"] = r.returncode
        rec["stdout"] = r.stdout[:120].decode(errors="replace")
        rec["stderr"] = r.stderr[:200].decode(errors="replace")
    except Exception as e:
        rec["rc"] = 2007
        rec["err"] = repr(e)
    return tag, name, rec

def main():
    jobs = [(t, b, n) for t, b in (("before", BEFORE), ("after", AFTER)) for n in NAMES]
    with ThreadPoolExecutor(max_workers=12) as ex:
        rows = list(ex.map(one, jobs))
    res = {"before": {}, "after": {}}
    for tag, name, rec in rows:
        res[tag][name] = rec
    json.dump(res, open("/tmp/opencode/d_js_compare_results.json", "w"), indent=1)

    bp = [n for n in NAMES if res["before"][n]["rc"] == 0]
    ap = [n for n in NAMES if res["after"][n]["rc"] == 0]
    p2f = [n for n in NAMES if res["before"][n]["rc"] == 0 and res["after"][n]["rc"] != 0]
    f2p = [n for n in NAMES if res["before"][n]["rc"] != 0 and res["after"][n]["rc"] == 0]
    print("JS total=%d" % len(NAMES))
    print("before pass=%d fail=%d" % (len(bp), len(NAMES) - len(bp)))
    print("after  pass=%d fail=%d" % (len(ap), len(NAMES) - len(ap)))
    print("pass->fail=%d %s" % (len(p2f), p2f))
    print("fail->pass=%d %s" % (len(f2p), f2p))
    for n in f2p:
        print("  %s before=%s(%r) after=%s(%r)" % (
            n, res["before"][n]["rc"], res["before"][n].get("stdout", ""),
            res["after"][n]["rc"], res["after"][n].get("stdout", "")))

if __name__ == "__main__":
    main()
