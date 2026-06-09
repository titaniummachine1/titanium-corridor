# Titanium Engine — Session State Handoff

**Purpose:** Carry context into a new chat without re-discovery.  
**Last updated:** search hardening + CAT view UI session (Jun 2026).

---

## Where we are

| Layer              | Status                                                                                                |
| ------------------ | ----------------------------------------------------------------------------------------------------- |
| **Perft**          | Depth 4 = **247_569_030** in **~3.4s** release — matches Ishtar/Canta. **Not** the search bottleneck. |
| **Movegen**        | Scalar hot path; `pawn_bits.rs` bitboard variant matches scalar (tests); not wired to perft yet.      |
| **Search**         | ID αβ + aspiration + LMR + qsearch + TT + CAT v3 prune. 73 tests pass.                                |
| **CAT overlay**    | Web shows raw cm on squares; engine API `titanium cat`.                                               |
| **Known weakness** | Depth 4+ search on heavy midgames still slow (~60k nps funnel); needs BFS cache.                      |

---

## What this session fixed

### Search correctness (why AI played weird moves)

1. **Timeout poisoning** — child `alpha` on stop → fake fail-high; partial ID depth committed.
2. **Root tie-break** — equal scores: passive wall beat path-gaining pawn (`f4v` vs `e4`).
3. **Mate extensions** — clamp-before-extend meant `mateExtensions` always 0.
4. **Gap zone bounds** — `0..=9` neighbor check → phantom mouths on board edges.
5. **Qsearch** — now path-noisy only (opp distance increases); not all legal walls.

### Search performance (depth regression was here, not perft)

| Before (bad)                              | After                                  |
| ----------------------------------------- | -------------------------------------- |
| `pawn_mobility` = full legal gen per eval | `generate_pawn_moves_for` only         |
| Opponent path built 3×/node               | Once, shared with collect + order      |
| CAT built 2×/node                         | Once (`depth ≥ 2`; qsearch skips)      |
| BFS per wall in ordering                  | Witness-path gate; BFS only if on path |
| Forcing ext at `dist ≤ 2`                 | `dist ≤ 1` or ≤1 pawn move             |
| `eval_stm` every child                    | Only unproven mate scores              |
| Qdepth 10, all walls                      | Qdepth 6, noisy walls, cap 8 moves     |

Funnel (`e5 e5v` line): **depth 3 in ~3s**, plays **`e4`** (was `f4v`).

### CAT v3 multi-route pruning

- **Problem:** Single witness shortest path = CAT v2 tunnel vision.
- **Now:** `wall_should_search` = on witness path **OR** `wall_edge_heat ≥ CAT_HOT_CM` (160).
- Gap/cross-gap walls: always searchable; sealed interior away from gap mouth: pruned.
- See `docs/video/CAT-v3-wall-shape.md`, `docs/video/11-search-hardening.md`.

### Web CAT vision

- Removed sharpness (γ) slider.
- Raw **cm** printed on each square; colors fixed to engine thresholds 60 / 160 / 240.
- Spec: `docs/video/CAT-VIEW-UI.md`.

---

## Architecture snapshot (current)

```
engine/src/
├── core/board.rs          Board, Move, zobrist, make/unmake
├── util/grid.rs           can_step, flood layout, wall bits
├── movegen/
│   ├── legal.rs           legal moves, WallPathCache, pawn slice
│   └── pawn_bits.rs       bitmask pawn experiment (test/bench)
├── path/                  BfsScratch, DirMasks, distance fill
├── cat/
│   ├── build.rs           CorridorAttention (multi-route heat)
│   ├── prune.rs           collect_search_moves, wall_should_search, ordering
│   ├── constants.rs       CAT_HOT=160, CAT_COLD=60, CAT_CORRIDOR=200
│   └── viz.rs             cat_snapshot_json for web
├── search/alphabeta.rs    ID negamax, qsearch, LMR, aspiration
└── util/perft.rs          perft_fast, PERFT4_STARTPOS
```

---

## CAT thresholds (current code)

| Constant          | Value | Use                                              |
| ----------------- | ----: | ------------------------------------------------ |
| `CAT_CORRIDOR_CM` |   200 | Per-player corridor ceiling                      |
| `CAT_HOT_CM`      |   160 | Tactical / skip LMR / prune gate for multi-route |
| `CAT_COLD_CM`     |    60 | Cold fringe / extra LMR / UI warm tint starts    |
| Display `maxCm`   |   240 | `CORRIDOR + BOTTLENECK_BONUS` for web ramp       |

---

## Regression tests

```bash
cd engine
cargo test --release                 # 73 passed, 2 ignored
cargo run --release -- perft 4       # 247569030 ~3.4s

# Funnel sanity
cargo run --release -- genmove --engine minimax --time 3 --log \
  e2 e8 e3 e7 e4 e6 d1h e6h d4 c6h d5 a6h e5 e5v
# expect: e4 (not passive wall)
```

Key tests: `funnel_position_avoids_tempo_waste`, `root_move_matches_best_scored_candidate_when_behind_with_walls`, `bitboard_matches_scalar_perft_depth3`, `gap_mouth_keeps_t_junction_tactics_prunes_deep_void`, CAT viz snapshot tests.

---

## Next priorities

1. **Per-node BFS cache** — parent distances; invalidate on wall moves only.
2. **Killer / history** — quiet move ordering (Sebastian ep. style).
3. **Wire pawn_bits** into hot path if bench shows win on real positions.
4. **Search depth 4 @ 3s** on midgame benchmarks (not just startpos perft).

---

## Video / docs index

| Doc                                                    | Content                             |
| ------------------------------------------------------ | ----------------------------------- |
| [11-search-hardening.md](video/11-search-hardening.md) | **This session** — episode script   |
| [BUG-DIARY.md](video/BUG-DIARY.md)                     | Entries #14–#21 added               |
| [CAT-VIEW-UI.md](video/CAT-VIEW-UI.md)                 | Web overlay spec                    |
| [CAT-SPEC.md](video/CAT-SPEC.md)                       | CAT v3 corridor (update thresholds) |
| [PERFT-BENCHMARKS.md](video/PERFT-BENCHMARKS.md)       | Perft still ~3.4s @ d4              |

---

## Commands cheatsheet

```bash
cd engine
cargo test --release
cargo run --release -- perft 4
cargo run --release -- bench
cargo run --release -- genmove --engine minimax --time 10 --log
cargo run --release -- cat e2 e8 e3

cd web && npm run dev   # CAT toggle in Analysis
```
