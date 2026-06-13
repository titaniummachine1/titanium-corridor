# Move generation — production architecture

**Status:** Closed for playing engine (Jun 2026, `movgen-o1-lookup` → `main`).  
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
| Wall emit | `movegen/legal.rs` | `collect_wall_orientation` (isolated phase → flood phase) |
| L3 flood | `path/parallel.rs` | `both_players_reach_goals_grids` |

**L2 and TOPO are shift algebra, not lookup tables.** Per-slot or global topo tables were tried and rejected (unsound or fake O(1)).

## Pawns

| Mode | Default? | Notes |
| ---- | -------- | ----- |
| `ShiftCanStep` | **Yes** | `pawn_bits.rs` — bit shift + `can_step` wall check |
| `O1Lookup` | Research | Offline `PAWN_LEGAL` tables; see pawn bench below |

## Performance (i7-4900MQ, release, 1 thread)

| Command | Result |
| ------- | ------ |
| `titanium perft 3` | **2_062_264** nodes |
| `titanium perft 4` | **247_569_030** nodes |
| `titanium bench 3 10` | ~**175–250M nps** (honest: movegen + make/unmake) |

Perft time uses bulk counting at depth 1 and TT — **not** comparable to bench nps.

### Pawn mode bench (`cargo bench --bench perft_pawn_modes`, perft 4, no TT)

| Mode | vs fastest |
| ---- | ---------- |
| `o1_lookup` | 1.00× |
| `scalar_can_step` | 1.04× |
| `shift_bit_can_step` (default) | 1.11× |
| `bitboard_*` | ~2.4× |

`ShiftCanStep` stays default: small perft gap vs O1, keeps 1.6MB pawn tables out of the hot cache alongside wall shift path. Revisit only with in-search profiling.

## Offline pawn tables

```bash
cargo run --release --bin movegen-o1-gen
```

Regenerates `src/movegen/o1/generated_tables_data.rs` + remap bins. **Wall tables removed** — generator is pawn-only.

## Regression

```bash
cargo test --release
cargo test --release movegen::o1::lookup
cargo test --release movegen::legal
cargo test --release perft_depth4_matches_oracle -- --ignored --nocapture
```

## What not to do

- Multithreaded movegen inside search nodes
- GPU wall flood
- Global / per-slot topo lookup tables
- `can_wall_block_topology` witness skip without proof harness (see BUG-DIARY `a1h`/`a5h`)

## Next work (not movegen)

See `docs/MOVEGEN-HANDOFF.md` — make/unmake, incremental L3, completeness oracle.
