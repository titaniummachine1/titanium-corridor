# Move generation — production architecture

**Status:** Closed for playing engine (Jun 2026).  
**Policy:** Single-thread hot path only — no movegen multithreading, no GPU.

## Wall pipeline

```text
L1  empty slot          !horizontal_walls / !vertical_walls
L2  collision           whole-board shifts (overlap / cross / neighbor)
TOPO flood-skip         two-of-three anchor shifts (= scraped canWallBlock)
L3  path legality       parallel u128 flood + bit theft (lazy WallTrialCtx)
```

| Layer | File | Function |
| ----- | ---- | -------- |
| L1∧L2∧TOPO masks | `movegen/o1/lookup.rs` | `wall_masks(board)` |
| Wall emit | `movegen/legal.rs` | `collect_wall_orientation` (isolated → flood) |
| L3 flood | `path/parallel.rs` | `both_players_reach_goals_grids` |

**Walls use shift algebra only** — no runtime wall tables (tried; rejected as fake or unsound).

## Pawns — production vs research

| Mode | Used in play? | How |
| ---- | ------------- | --- |
| **`ShiftCanStep`** | **Yes — default** | `PawnGenMode::default()` in `legal.rs`; search, CLI, perft |
| **`O1Lookup`** | **No — research** | `PawnGenMode::O1Lookup` only when explicitly selected (benches, tests) |

### Why we built O1 pawn tables but don't use them in production

The `movgen-o1-lookup` branch originally targeted O(1) lookup for **both** pawns and walls:

- **Walls:** moved to **shift masks** (faster, correct) — tables removed.
- **Pawns:** offline `PAWN_LEGAL[sq][enemy][wall]` tables were generated and **verified** against scalar (tests pass), but **`ShiftCanStep` stayed the default** because:
  1. **Cache:** ~1.6MB `generated_remap.bin` + table data competes with the hot wall shift path in the same node.
  2. **Simplicity:** production movegen needs no generator step; `ShiftCanStep` is a few bit shifts + `can_step`.
  3. **Bench is noisy:** isolated `perft_pawn_modes` (d4, no TT) sometimes shows O1 slightly ahead, sometimes shift — not decisive enough to justify the table footprint in search.

**O1 is not deleted.** It remains for:

- Table correctness tests (`movegen::o1::lookup`)
- `cargo run --bin movegen-o1-gen` (regenerate tables)
- Future completeness / invariant experiments that may need precomputed pawn masks

To run with O1 pawns explicitly: `perft_no_tt_mode(..., PawnGenMode::O1Lookup)` or extend CLI — not wired to `titanium` default.

## Performance (i7-4900MQ, release, 1 thread, Jun 2026)

| Command | Result |
| ------- | ------ |
| `titanium perft 3` | **2_062_264** nodes |
| `titanium perft 4` | **247_569_030** nodes (~0.30–0.35s CLI, TT + bulk d1) |
| `titanium bench 3 20` | ~**210–240M nps** (honest: movegen + make/unmake + Zobrist) |

Perft CLI time ≠ bench nps (bulk depth-1 + TT).

### Pawn mode bench (`cargo bench --bench perft_pawn_modes`, perft 4, no TT)

Runs vary with CPU load; all modes must match **247_569_030** nodes.

| Mode | Typical |
| ---- | ------- |
| `shift_bit_can_step` (default) | ~1.0–1.2× vs fastest |
| `o1_lookup` | ~1.0–1.2× vs fastest |
| `scalar_can_step` | ~1.0–1.05× |
| `bitboard_*` | ~2.5× slower |

**Decision:** keep **`ShiftCanStep` default** regardless of which wins a given run — cache policy + no table dependency.

## Offline pawn tables (research)

```bash
cargo run --release --bin movegen-o1-gen
```

Output: `src/movegen/o1/generated_tables_data.rs`, `generated_remap.bin`. Not required for `cargo build` if files are committed.

## Regression

```bash
cargo test --release
cargo test --release movegen::o1::lookup
cargo test --release perft_depth4_matches_oracle -- --ignored --nocapture
cargo run --release --bin titanium -- bench 3 20
```

## Next work

See `docs/MOVEGEN-HANDOFF.md` — L3 profiling in search, completeness oracle (not movegen).
