# Move generation — production architecture

**Status:** Closed for playing engine (Jun 2026).  
**Policy:** Single-thread hot path only — no movegen multithreading, no GPU.

## Wall pipeline (production — the real speedup)

```text
L1  empty slot          !horizontal_walls / !vertical_walls
L2  collision           whole-board shifts (overlap / cross / neighbor)
TOPO flood-skip         two-of-three anchor shifts (= scraped canWallBlock)
L3  path legality       parallel u128 flood + bit theft (lazy WallTrialCtx)
```

| Layer | File | Function |
| ----- | ---- | -------- |
| L1∧L2∧TOPO masks | **`movegen/wall_masks.rs`** | `wall_masks(board)` |
| Wall emit | `movegen/legal.rs` | `collect_wall_orientation` (isolated → flood) |
| L3 flood | `path/parallel.rs` | `both_players_reach_goals_grids` |

**Walls use shift algebra** — whole-board u64 shifts compute L2 collision + TOPO flood-skip in ~6 ops per orientation. This is the measured perft speedup (210–230M+ nps). No runtime wall **tables** (global topo tables were tried and rejected as unsound).

## Pawns — production only

| Mode | Used in play? | How |
| ---- | ------------- | --- |
| **`ShiftCanStep`** | **Yes — default** | `PawnGenMode::default()` in `legal.rs`; search, CLI, perft |

Pawn gen is a few bit shifts + `can_step` — already cheap. No tables on `main`.

### Pawn O1 tables — research branch only

The ~2MB `PAWN_LEGAL` lookup (`generated_remap.bin` + tables) lives on branch **`movgen-o1-lookup`** only.

Measured on pawn-only perft (no walls): O1 beats `ShiftCanStep` by **~3–4%** — not worth 2MB cache pressure next to the hot wall shift path in real search.

```bash
git checkout movgen-o1-lookup   # full O1 pawn tables + movegen-o1-gen + pawn-only bench
```

## Performance (i7-4900MQ, release, 1 thread, pinned core 7)

| Command | Result |
| ------- | ------ |
| `titanium perft 3` | **2_062_264** nodes |
| `titanium perft 4` | **247_569_030** nodes (~0.32s pinned) |
| `titanium bench 3 20` | ~**231M nps** (honest: movegen + make/unmake + Zobrist) |

Reliable perf: `scripts/bench-pinned.ps1` or `$env:TITANIUM_PIN_CORE='7'`.

## Regression

```bash
cargo test --release
cargo test --release movegen::wall_masks
cargo test --release perft_depth4_matches_oracle -- --ignored --nocapture
powershell -File scripts/bench-pinned.ps1
```

## Next work

See `docs/MOVEGEN-HANDOFF.md` — L3 profiling in search, completeness oracle.
