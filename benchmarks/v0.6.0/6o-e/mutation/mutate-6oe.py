#!/usr/bin/env python3
"""6O-E mutation runner: apply each mutant to a scratch worktree, check it
applied and compiles, run the given test filters, record killed/survived,
revert. A mutant whose text is not found, or that does not compile, is
reported as NOT APPLIED / INVALID, never as killed.

usage: mutate-6oe.py WORKTREE MUTANTS.json OUT.log FILTER...
"""
import json, subprocess, sys, os
wt, spec, out = sys.argv[1], sys.argv[2], sys.argv[3]
filters = sys.argv[4:]
env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"],
           CARGO_TARGET_DIR=os.path.expanduser("~/recached-6oe-bench/target-") + os.path.basename(os.path.normpath(wt)))
log = open(out, "a")
def say(s):
    print(s); log.write(s + "\n"); log.flush()
subprocess.run(["git", "-C", wt, "checkout", "--", "."], check=True)
base = subprocess.run(["cargo", "test", "-p", "recached", "--bin", "recached-server", "--"] + filters +
                      ["--test-threads=8"], cwd=wt, env=env, capture_output=True, text=True, timeout=1800)
bres = [l for l in base.stdout.splitlines() if l.startswith("test result")]
say(f"BASELINE {subprocess.run(['git','-C',wt,'rev-parse','--short','HEAD'],capture_output=True,text=True).stdout.strip()} target={env['CARGO_TARGET_DIR']}: {'PASS' if base.returncode == 0 else 'FAIL'} {bres[-1] if bres else base.stderr[-800:]}")
if base.returncode != 0:
    sys.exit("baseline does not pass; no mutant can be judged")
for m in json.load(open(spec)):
    subprocess.run(["git", "-C", wt, "checkout", "--", "."], check=True)
    ok = True
    for e in m["edits"]:
        p = os.path.join(wt, e["file"]); s = open(p).read()
        n = s.count(e["old"])
        if n != e.get("count", 1):
            say(f"{m['id']}: NOT APPLIED ({e['file']}: found {n}x) — {m['what']}"); ok = False; break
        s = s.replace(e["old"], e["new"]); open(p, "w").write(s)
    if not ok:
        continue
    b = subprocess.run(["cargo", "test", "-p", "recached", "--bin", "recached-server", "--no-run"],
                       cwd=wt, env=env, capture_output=True, text=True)
    if b.returncode != 0:
        say(f"{m['id']}: INVALID (does not compile) — {m['what']}\n{b.stderr[-1500:]}"); continue
    r = subprocess.run(["cargo", "test", "-p", "recached", "--bin", "recached-server", "--"] + filters +
                       ["--test-threads=8"], cwd=wt, env=env, capture_output=True, text=True, timeout=1800)
    failed = [l.split()[1] for l in r.stdout.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    res = [l for l in r.stdout.splitlines() if l.startswith("test result")]
    if r.returncode != 0:
        say(f"{m['id']}: KILLED by {len(failed)} ({', '.join(failed[:4])}{'…' if len(failed)>4 else ''}) — {m['what']}")
    else:
        say(f"{m['id']}: SURVIVED ({res[-1] if res else ''}) — {m['what']}")
subprocess.run(["git", "-C", wt, "checkout", "--", "."], check=True)
