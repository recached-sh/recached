exec(open("/home/dennispaler/recached-6oh-bench/mutants-pr3.py").read())
MUTANTS = [m for m in MUTANTS if m[0].split()[0] in ("M7", "M10")]
