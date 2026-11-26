#!/usr/bin/env python3
"""Apply each server mutant in a worktree, run the given test filters, report
killed/survived, and restore the committed files (git show HEAD:<file>).

usage: mutate-server.py <worktree> <mutants.py> [test filter ...]
mutants.py defines MUTANTS = [(name, file, old, new), ...]
"""
import os, subprocess, sys

wt, spec, *filters = sys.argv[1:]
ns = {}
exec(open(spec).read(), ns)
env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"],
           RECACHED_REPRODUCE_6OF_COUNTER="1")
filters = filters or ["expiry"]
for name, path, old, new in ns["MUTANTS"]:
    full = os.path.join(wt, path)
    src = open(full).read()
    if src.count(old) != 1:
        print(f"SKIP     {name}: pattern found {src.count(old)} times", flush=True)
        continue
    open(full, "w").write(src.replace(old, new))
    try:
        r = subprocess.run(["cargo", "test", "-q", "-p", "recached", "--target-dir",
                            os.path.join(wt, "target"), "--", *filters],
                           cwd=wt, env=env, capture_output=True, text=True)
        out = r.stdout + r.stderr
        if r.returncode == 0:
            print(f"SURVIVED {name}", flush=True)
        else:
            failed = [l.split()[0] for l in out.splitlines() if l.endswith("FAILED") and l.startswith("    ")]
            if "error[E" in out:
                print(f"BUILD    {name}: mutant does not compile", flush=True)
            else:
                print(f"killed   {name}: {', '.join(failed[:4]) or 'failed'}", flush=True)
    finally:
        restored = subprocess.run(["git", "show", f"HEAD:{path}"], cwd=wt, capture_output=True,
                                  text=True, check=True).stdout
        open(full, "w").write(restored)
