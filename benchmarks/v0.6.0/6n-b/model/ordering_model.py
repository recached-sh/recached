#!/usr/bin/env python3
"""Exhaustive interleaving model of watermark-gated fan-out (Phase 6N-B).

A design aid, not production code. It checks every interleaving of a small
set of processes that share one key `k` and one write-order stripe:

  W1, W2  INCR k: take the stripe, apply, capture effects, stage the AOF
          frame (commit seq), release the stripe, then wait for the reply gate.
  L       the AOF leader: perform the open batch (the written watermark
          advances), then release its effects in seq order (the replica frame,
          the WATCH keychange).
  E       EXEC of a transaction that WATCHed k when k was 0: take the stripe,
          check dirtiness, and commit only if clean.
  F       a replica full sync: take every stripe, (optionally) drain, snapshot
          the store, register the replica's channel.

Properties checked in every reachable state:

  P1 ack => released     a reply never precedes its write's effect release
  P2 visible => written  nothing reaches an observer before its AOF write
                         (frames, keychanges, and a full-sync snapshot)
  P3 WATCH/EXEC          EXEC never commits after a write to k that it should
                         have seen
  P4 replica converges   at quiescence the replica equals the primary
                         (a write in the snapshot is never streamed again)

Variants:

  inline        today: effects released before the writer drops its stripe
  naive         stage and drop the stripe; no other change
  dirty-only    + WATCH dirtiness set at capture time, under the stripe
  drain-only    + full sync waits for released == issued under its barrier
  dirty+drain   both, but the reply gate still keys on the written watermark
  fixed         both, and a batch completes (releasing its replies) only
                after its effects are released

Run:  python3 -I ordering_model.py
"""

from __future__ import annotations

import sys
from collections import deque
from dataclasses import dataclass, replace
from typing import FrozenSet, Optional, Tuple


@dataclass(frozen=True)
class Variant:
    name: str
    inline: bool  # writers hold the stripe until their effects are released
    dirty_split: bool  # WATCH dirty flag set at capture, not at delivery
    drain: bool  # full-sync barrier waits for released == issued
    ack_on_release: bool = False  # reply gate waits for batch completion,
    # which follows release, rather than for the written watermark


VARIANTS = [
    Variant("inline", inline=True, dirty_split=False, drain=False),
    Variant("naive", inline=False, dirty_split=False, drain=False),
    Variant("dirty-only", inline=False, dirty_split=True, drain=False),
    Variant("drain-only", inline=False, dirty_split=False, drain=True),
    Variant("dirty+drain", inline=False, dirty_split=True, drain=True),
    Variant("fixed", inline=False, dirty_split=True, drain=True, ack_on_release=True),
]


@dataclass(frozen=True)
class S:
    store: int = 0
    stripe: Optional[str] = None
    next_seq: int = 1
    open_batch: Tuple[Tuple[int, str], ...] = ()  # staged (seq, op), not yet taken by L
    in_flight: Tuple[Tuple[int, str], ...] = ()  # written by L, not yet released
    written: int = 0
    released: int = 0
    # WATCH (E's connection)
    dirty_atomic: bool = False
    notif_channel: int = 0  # keychanges delivered to E's connection
    # replica
    registered: bool = False
    replica: int = 0
    # program counters and per-writer seq
    pc: Tuple[Tuple[str, int], ...] = (("W1", 0), ("W2", 0), ("E", 0), ("F", 0))
    seq_of: Tuple[Tuple[str, int], ...] = ()
    acked: FrozenSet[str] = frozenset()
    exec_committed: bool = False

    def pc_of(self, p: str) -> int:
        return dict(self.pc)[p]

    def with_pc(self, p: str, v: int) -> "S":
        d = dict(self.pc)
        d[p] = v
        return replace(self, pc=tuple(sorted(d.items())))

    def seq(self, p: str) -> int:
        return dict(self.seq_of)[p]


DONE = 99


def steps(s: S, v: Variant):
    """Yield (label, next_state, violation_or_None)."""
    # Writers
    for w in ("W1", "W2"):
        pc = s.pc_of(w)
        if pc == 0 and s.stripe is None:
            yield f"{w}.lock", replace(s.with_pc(w, 1), stripe=w), None
        elif pc == 1:
            # apply + capture + stage, one step under the stripe
            n = s.with_pc(w, 2)
            seq = s.next_seq
            dirty = s.dirty_atomic or (v.dirty_split and s.pc_of("E") == 0)
            n = replace(
                n,
                store=s.store + 1,
                next_seq=seq + 1,
                open_batch=s.open_batch + ((seq, "incr"),),
                seq_of=s.seq_of + ((w, seq),),
                dirty_atomic=dirty,
            )
            yield f"{w}.apply+stage", n, None
        elif pc == 2:
            if v.inline and s.released < s.seq(w):
                continue  # today: hold the stripe until fanned out
            yield f"{w}.unlock", replace(s.with_pc(w, 3), stripe=None), None
        elif pc == 3 and (s.released if v.ack_on_release else s.written) >= s.seq(w):
            bad = None
            if s.released < s.seq(w):
                bad = f"P1: {w} acknowledged before its effects were released"
            yield f"{w}.ack", replace(s.with_pc(w, DONE), acked=s.acked | {w}), bad

    # Leader: perform the open batch, then release it
    if s.open_batch and not s.in_flight:
        yield "L.perform", replace(
            s,
            written=max(q for q, _ in s.open_batch),
            in_flight=s.open_batch,
            open_batch=(),
        ), None
    if s.in_flight:
        bad = None
        if any(q > s.written for q, _ in s.in_flight):
            bad = "P2: effect released before its AOF write"
        replica = s.replica
        if s.registered:
            for _, op in s.in_flight:
                replica = replica + 1 if op == "incr" else 100
        n = replace(
            s,
            released=max(q for q, _ in s.in_flight),
            in_flight=(),
            replica=replica,
            # E's WATCH keychange is delivered at release time; it only
            # reaches E while E is still watching (before EXEC ran).
            notif_channel=s.notif_channel
            + (len(s.in_flight) if s.pc_of("E") == 0 else 0),
        )
        yield "L.release", n, bad

    # EXEC
    pc = s.pc_of("E")
    if pc == 0 and s.stripe is None:
        yield "E.lock", replace(s.with_pc("E", 1), stripe="E"), None
    elif pc == 1:
        dirty = s.dirty_atomic if v.dirty_split else s.notif_channel > 0
        bad = None
        n = s
        if not dirty:
            if s.store != 0:
                bad = "P3: EXEC committed although a write to the watched key preceded it"
            n = replace(s, store=100, exec_committed=True)
            # EXEC's own write: staged like any write (same stripe held)
            seq = n.next_seq
            n = replace(
                n,
                next_seq=seq + 1,
                open_batch=n.open_batch + ((seq, "set100"),),
                seq_of=n.seq_of + (("E", seq),),
            )
        yield "E.check+exec", n.with_pc("E", 3), bad
    elif pc == 3:
        mine = dict(s.seq_of).get("E", 0)
        if not (v.inline and s.released < mine):
            yield "E.unlock", replace(s.with_pc("E", DONE), stripe=None), None

    # Full sync
    pc = s.pc_of("F")
    if pc == 0 and s.stripe is None:
        yield "F.lock_all", replace(s.with_pc("F", 1), stripe="F"), None
    elif pc == 1:
        issued = s.next_seq - 1
        if v.drain and s.released < issued:
            pass  # wait for the leader, still holding the barrier
        else:
            bad = None
            if s.written < issued:
                bad = "P2: full-sync snapshot exposes a write not yet in the AOF"
            # The snapshot holds every applied write. EXEC's SET makes the
            # value absolute; INCRs are relative, so a re-streamed INCR shows.
            n = replace(s, registered=True, replica=s.store, stripe=None)
            # Frames already released before registration are in the snapshot;
            # frames released after it are streamed. With staging, a staged
            # but unreleased frame is in BOTH (P4 catches it at quiescence).
            yield "F.snapshot+register", n.with_pc("F", DONE), bad


def quiescent(s: S) -> bool:
    return (
        all(p == DONE for _, p in s.pc)
        and not s.open_batch
        and not s.in_flight
    )


def check(v: Variant):
    start = S()
    seen = {start}
    q = deque([(start, ())])
    violations: dict[str, Tuple[str, ...]] = {}
    states = 0
    while q:
        s, trace = q.popleft()
        states += 1
        progressed = False
        for label, n, bad in steps(s, v):
            progressed = True
            t = trace + (label,)
            if bad and bad not in violations:
                violations[bad] = t
            if n not in seen:
                seen.add(n)
                q.append((n, t))
        if quiescent(s) and s.registered and s.replica != s.store:
            msg = (
                "P4: replica diverged at quiescence "
                f"(primary {s.store}, replica {s.replica})"
            )
            violations.setdefault(msg, trace)
        if not progressed and not quiescent(s):
            violations.setdefault("deadlock", trace)
    return states, violations


def main() -> int:
    expected_clean = {"inline", "fixed"}
    ok = True
    for v in VARIANTS:
        states, violations = check(v)
        print(f"== {v.name}: {states} states, {len(violations)} violation kind(s)")
        for msg, trace in sorted(violations.items()):
            print(f"   {msg}")
            print(f"     e.g. {' -> '.join(trace)}")
        clean = not violations
        if clean != (v.name in expected_clean):
            ok = False
    print("RESULT:", "as expected" if ok else "UNEXPECTED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
