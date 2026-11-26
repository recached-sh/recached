#!/usr/bin/env python3
"""Apply each mutant (a list of (file, old, new) edits) in a worktree, run the
test filters, report killed / SURVIVED / BUILD (does not compile) / SKIP, and
restore the committed files. An uncompiled mutant is never counted as killed.

usage: run-mutants.py <worktree> <mutants.py> [test filter ...]
"""
import os, subprocess, sys, time

wt, spec, *filters = sys.argv[1:]
ns = {}
exec(open(spec).read(), ns)
env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])
for name, edits in ns["MUTANTS"]:
    files = {}
    ok = True
    for path, old, new in edits:
        full = os.path.join(wt, path)
        src = files.get(full) or open(full).read()
        if src.count(old) != 1:
            print(f"SKIP     {name}: pattern found {src.count(old)} times in {path}", flush=True)
            ok = False
            break
        files[full] = src.replace(old, new)
    if not ok:
        continue
    for full, src in files.items():
        open(full, "w").write(src)
    started = time.time()
    try:
        r = subprocess.run(["cargo", "test", "-q", "--locked", "-p", "recached", "--bin", "recached-server",
                            "--", *filters], cwd=wt, env=env, capture_output=True, text=True)
        out = r.stdout + r.stderr
        took = int(time.time() - started)
        if "error[E" in out or "could not compile" in out:
            print(f"BUILD    {name}: mutant does not compile ({took}s)", flush=True)
        elif r.returncode == 0:
            print(f"SURVIVED {name} ({took}s)", flush=True)
        else:
            failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
            print(f"killed   {name}: {', '.join(failed[:3]) or 'failed'} ({took}s)", flush=True)
    finally:
        for full in files:
            rel = os.path.relpath(full, wt)
            restored = subprocess.run(["git", "show", f"HEAD:{rel}"], cwd=wt, capture_output=True,
                                      text=True, check=True).stdout
            open(full, "w").write(restored)
