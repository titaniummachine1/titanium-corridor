# Episode 06 — Threading prep (Titanium vs Titanium)

- **branch:** `checkpoint/06-threading`
- **commit:** `098477c`
- **tag:** `checkpoint-06-threading`

**Status:** architecture landed; Lazy SMP search = later episode. Tournament registry: [TOURNAMENT-ROADMAP.md](TOURNAMENT-ROADMAP.md).

## Hook

"We made perft fast on one core. Before αβ search, we lay the engine room out like Stockfish — shared hash table, per-thread scratch — and run **Titanium vs Titanium** on the benchmark stage."

## What we built (not full Lazy SMP yet)

```text
SharedState     → TT (one day: Arc + atomic stores for Lazy SMP)
WorkerContext   → BfsScratch (one per thread, never shared)
Engine          → limits.threads (default 1 = deterministic tests)
```

| `threads` | Behaviour                                          |
| --------- | -------------------------------------------------- |
| `1`       | Single-thread perft — **CI / correctness**         |
| `N > 1`   | Root-split parallel perft — **bench / video only** |

Root parallel = split ~131 root moves across cores. Each subtree has its own TT. **Same node count**, faster wall clock.

**Not yet:** Lazy SMP αβ (N independent searches sharing one TT). That's the next threading episode after search exists.

## Demo commands

```bash
cd engine

# Correctness path (always threads=1 in tests)
cargo test

# Single-thread timing
cargo run --release -- perft 3

# Parallel bench (video money shot — use depth 4; depth 3 subtrees are too small)
cargo run --release -- thread-bench 4
cargo run --release -- thread-bench 4 --threads 8

# Any command accepts --threads
cargo run --release -- bench 3 20 --threads 4
```

**Discovery #E:** Root parallel at **depth 3** often **slows down** — 131 tiny subtrees, thread overhead wins. Use **depth 4** for the benchmark episode.

**Discovery #F:** Each parallel worker must **not** allocate its own full TT — that was 131× megabyte heap churn. Parallel subtrees run with `TT = None`.

**Discovery #G:** `thread-bench` compares **no-TT** single vs parallel — otherwise single-thread TT makes the comparison unfair (same node count, less CPU work).

(Times vary by CPU — speedup not linear because wall BFS + memory bandwidth.)

## Video beats

1. Show `thread-bench` — same 2_062_264 nodes, different times.
2. "We didn't parallelize the tree — we parallelized **root moves**. Search will use Stockfish Lazy SMP later."
3. `cargo test` still passes — production path is `threads=1`.
4. Architecture diagram: one `SharedState`, N `WorkerContext`s.

## Lesson

Prepare threading **shape** early (no globals, explicit contexts). Implement **parallelism** only where it's simple and testable. Root parallel perft is the sandbox before Lazy SMP αβ.

## Next episode

αβ + distance eval (single-thread), then Lazy SMP search reusing `Engine` + `SharedState`.
