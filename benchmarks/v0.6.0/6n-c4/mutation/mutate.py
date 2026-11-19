import subprocess, sys, os, shutil
root = sys.argv[1]
out = sys.argv[2]
P = os.path.join(root, 'server-native/src/persistence.rs')
orig = open(P).read()
M = {
 'M1-old-heuristic': ('let gained = done.frames() > position;', 'let gained = done.is_complete() || done.frames() > position;'),
 'M2-gain-on-equal': ('let gained = done.frames() > position;', 'let gained = done.frames() >= position;'),
 'M3-always-lead': ('        if done.is_complete() {\n            // Another writer, resuming first', '        if false {\n            // Another writer, resuming first'),
 'M4-never-lead-after-queue': ('        } else {\n            shared.lead();\n        }\n        gate_wait(shared, &done).await', '        } else {\n        }\n        gate_wait(shared, &done).await'),
 'M5-no-frame-mirror': ('        self.done\n            .frames\n            .store(self.frames, std::sync::atomic::Ordering::Relaxed);', ''),
 'M6-position-before-push': ('            batch.push_frame(resp);\n            position = batch.frames;', '            position = batch.frames;\n            batch.push_frame(resp);'),
 'M7-no-backoff': ('            self.skip.store(COALESCE_BACKOFF, Relaxed);\n            bump(backoff_count, 1);', '            bump(backoff_count, 1);'),
 'M8-no-cancel-guard': ('            lead_on_drop.disarm();\n            // A gain', '            lead_on_drop.disarm();\n            // A gain'),
}
# M8: disarm the guard before yielding
M['M8-no-cancel-guard'] = ('            let lead_on_drop = LeadOnDrop(Some(shared));\n            tokio::task::yield_now().await;\n            lead_on_drop.disarm();', '            let lead_on_drop = LeadOnDrop(Some(shared));\n            lead_on_drop.disarm();\n            tokio::task::yield_now().await;')
env = dict(os.environ, PATH=os.path.expanduser('~/.cargo/bin') + ':' + os.environ['PATH'])
with open(out, 'w') as log:
    for name, (a, b) in M.items():
        assert orig.count(a) == 1, name
        open(P, 'w').write(orig.replace(a, b))
        try:
            r = subprocess.run(['cargo', 'test', '-p', 'recached', '--all-features', '--bins', 'coalesced_fanout', '--', '--test-threads=4'],
                               cwd=root, env=env, capture_output=True, text=True, timeout=900)
            txt = r.stdout + r.stderr
        except subprocess.TimeoutExpired as e:
            txt = 'TIMEOUT\n' + (e.stdout or b'').decode(errors='replace') if isinstance(e.stdout, bytes) else 'TIMEOUT'
        failed = [l.split()[1] for l in txt.splitlines() if l.startswith('test ') and l.rstrip().endswith('FAILED')]
        summary = [l for l in txt.splitlines() if 'test result' in l or l.startswith('error[')]
        log.write(f'{name}: {len(failed)} failed: {failed}\n  {summary}\n'); log.flush()
    open(P, 'w').write(orig)
