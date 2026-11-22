#!/usr/bin/env python3
"""Run a command and print its rusage (children included) on stderr's last line.

    rusage.py <out-file> -- <command ...>

Writes "user_s=.. sys_s=.. nvcsw=.. nivcsw=.. maxrss_kb=.." to <out-file>:
the generator's context switches show whether its threads slept between
replies (Phase 6O-B; the host has no /usr/bin/time).
"""
import os
import subprocess
import sys

out, sep, cmd = sys.argv[1], sys.argv[2], sys.argv[3:]
assert sep == "--" and cmd, __doc__
p = subprocess.Popen(cmd)
_, status, ru = os.wait4(p.pid, 0)
with open(out, "w") as f:
    f.write(f"user_s={ru.ru_utime:.3f} sys_s={ru.ru_stime:.3f} nvcsw={ru.ru_nvcsw} nivcsw={ru.ru_nivcsw} maxrss_kb={ru.ru_maxrss}\n")
sys.exit(os.waitstatus_to_exitcode(status))
