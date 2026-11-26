import subprocess, sys, os
SRC = "~/Documents/OpenSource/recached-6og-store/core-engine/src/store.rs"
orig = open(SRC).read()
M = {
 "M1_mark_after_first_write": [
   ("""        for stripe in &stripes {
            stripe.mark(SEQ_ACTIVE | SEQ_MULTI);
        }
""", ""),
   ("""            if ran.is_err() {
                applied.panicked = Some(i);""",
    """            if span.is_some_and(|(first, _)| i == first) { if let Some(h) = &hold { for s in &h._stripes { s.mark(SEQ_ACTIVE | SEQ_MULTI); } } }
            if ran.is_err() {
                applied.panicked = Some(i);"""),
 ],
 "M2_release_before_last_write": [
   ("""                hold = Some(self.hold_transaction(held, capped));
            }
""", """                hold = Some(self.hold_transaction(held, capped));
            }
            if span.is_some_and(|(_, last)| i == last) { hold = None; }
""")],
 "M3_inscope_read_one_waits": [
   ("""                if self.scope().is_some() {
                    return f();
                }
                #[cfg(any(test, feature = "test-hooks"))]
                write_pause::reader_blocked(self.hook_id(), &[stripe], false);""",
    """                #[cfg(any(test, feature = "test-hooks"))]
                write_pause::reader_blocked(self.hook_id(), &[stripe], false);""")],
 "M3b_inscope_read_many_waits": [
   ("""            if busy && self.scope().is_some() {
                return f();
            }""", "")],
 "M4_reserved_in_scope": [
   ("""            || !probe_eligible(cmd.borrow())
            || self.scope().is_some()
        {""", """            || !probe_eligible(cmd.borrow())
        {""")],
 "M5_release_before_repair": [
   ("self.repair_after_panic(hold.take());", "drop(hold.take()); self.repair_after_panic(None);")],
 "M6_no_repair": [
   ("self.repair_after_panic(hold.take());", "let _ = &hold;")],
 "M7_gate_again_in_scope": [
   ("(enforce && in_tx.is_none()).then", "enforce.then")],
 "M8_no_gate_in_capped_tx": [
   ("let gate = capped.then(|| {", "let gate = false.then(|| {")],
 "M9_capped_subset_stripes": [
   ("if all || capped {", "if all {")],
 "M10_unmarked_hold": [
   ("""        for stripe in &stripes {
            stripe.mark(SEQ_ACTIVE | SEQ_MULTI);
        }
""", """        for stripe in &stripes {
            stripe.mark(SEQ_ACTIVE);
        }
""")],
 "M11_no_catch_unwind_scope_kept": [
   ("let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {", "let ran: Result<(), ()> = Ok(()); (|| {"),
   ("""                    write_pause::transaction_step(self.hook_id(), &keys);
                }
            }));""", """                    write_pause::transaction_step(self.hook_id(), &keys);
                }
            })();"""),
 ],
}
names = sys.argv[1:] or list(M)
env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin:") + os.environ["PATH"], CARGO_TARGET_DIR="~/recached-6og-bench/target-mut-store")
filt = os.environ.get("FILTER", "transaction_tests")
try:
    for name in names:
        s = orig
        for a, b in M[name]:
            assert s.count(a) == 1, (name, a[:60], s.count(a))
            s = s.replace(a, b)
        open(SRC, "w").write(s)
        r = subprocess.run(["timeout", "600", "cargo", "test", "-q", "-p", "core-engine", "--features", "test-hooks", "--lib", filt],
                           cwd="~/Documents/OpenSource/recached-6og-store", env=env, capture_output=True, text=True)
        out = r.stdout + r.stderr
        lines = [l for l in out.splitlines() if "test result" in l or "FAILED" in l or "panicked" in l or "error" in l.lower()[:6]]
        print(f"=== {name}: rc={r.returncode}")
        print("\n".join(lines[:15]))
        sys.stdout.flush()
finally:
    open(SRC, "w").write(orig)
