# Titanium Engine — Session State Handoff

**Purpose:** Carry all important context into a new chat without paying re-discovery costs.  
**Last updated:** session ending after Layer 4 (bitwise flood fill + CAT integration).

---

## Where we are right now

All uncommitted changes are in the `engine/src/` tree (see `git diff --stat HEAD`).  
They are **correct and tested** — perft 4 matches the Ishtar/Canta oracle.

---

## What was implemented (this session)

### Layer 4A — Bitwise flood fill (`path.rs`, `grid.rs`)

Replaced the queue-based BFS in `path.rs` with a word-parallel bitwise flood fill.

**Key data structures:**
- `DirMasks { north, south, east, west: u128 }` — built once per wall snapshot by iterating all 81 squares and calling `can_step`. Bit `sq` is set iff the pawn at `sq` may move in that direction.
- `expand_frontier(frontier, masks) -> u128` — one shift per direction, OR together. **No loop.**
- `flood_to_goal(start_sq, masks, goal_mask) -> (bool, u128)` — returns reachability + full component mask.

**Centered 11-wide `u128` layout** (`grid.rs`):
- `FLOOD_STRIDE = 11`, `FLOOD_COL_PAD = 1`, `FLOOD_ROW_PAD = 1`.
- Playable `(row, col)` → bit `(row+1)*11 + (col+1)`. Max bit = 108 (fits u128).
- The 1-column left/right buffers mean an east/west shift from a board edge lands in a padding column, not the adjacent row. **No per-expand boundary checks needed.**
- `FLOOD_PLAYABLE: u128` — compile-time constant bitmask of all 81 playable squares.
- `pack_flood_mask(u128) -> u128` — converts internal flood bits back to 81-bit compact mask (API boundary only, never called in the hot path).

**Ishtar component reuse:**
- `both_players_reach_goals` runs flood fill for P1 first; if P2's pawn bit is inside P1's flood component AND P1's goal row is also reachable, P1's component already proves connectivity for both — P2 flood is skipped.

**Performance (release, this machine):**

| Depth | Nodes            | Time      |
|-------|------------------|-----------|
| 1     | 131              | —         |
| 2     | 16,677           | —         |
| 3     | 2,062,264        | ~0.06s    |
| 4     | **247,569,030**  | **~3.1s** |

Depth-4 oracle locked as `PERFT4_STARTPOS = 247_569_030` in `perft.rs`.

---

### Layer 4B — Known-path wall skip (`moves.rs`)

`WallPathCache` holds both players' shortest paths (arrays of square indices, computed via BFS once per position). Wall legality now has 4 gates:

1. `wall_collides` — instant reject (overlapping wall).
2. `can_wall_block_topology` → `false` means wall can't block anything, accept immediately.
3. If wall doesn't intersect either player's current shortest path (`wall_intersects_either_path`) → accept immediately (neither path is cut).
4. Fallback to `path_ok_after_wall` (full flood trial) — only walls that touch at least one path AND could form a closed cage reach here.

`WallPathCache` is **lazy**: built only when `can_wall_block_topology` returns true. At startpos (open board) the cache is never allocated during perft. Midgame, one cache per position, amortised over all wall candidates.

---

### CAT — Consensus Attention Table (`path.rs`, `search.rs`)

CAT is **search-only**, never in perft. It is built once per search node, not per move.

**What it is:** Per-square attention score (centi-units, `u16`), accumulated by running both players' level-BFS passes in `build_consensus_attention`. Each reached square accumulates `attention_weight_cm(dist)` forward; squares on the reconstructed shortest path receive a second identical accumulation (back-propagation).

**Formula:**
```
attention_weight_cm(dist) = 100 - min(dist * 3, 30)
```
- On-path, adjacent (dist=0/1): 100 + 100 = **200 cm**
- On-path, far (dist≥10): 70 + 70 = **140 cm**
- Off-path, adjacent: **100 cm**
- Off-path, far: **70 cm**

**Usage in search (`search.rs`):**
- `CAT_HOT_CM = 180` — skip LMR for moves on hot squares (tactical).
- `CAT_COLD_CM = 80` — add 1 extra ply of reduction for moves on cold squares.
- CAT score also replaces `TEMPO_PENALTY` as a tie-breaker in `move_order_score` for quiet wall moves.

**Why CAT is not "free" anymore:** The original design envisioned CAT being computed during the same BFS that wall legality uses in move generation, making it zero-cost. After splitting the bitwise flood into a wall-legality-only fast path (which does not do level-BFS with parent tracking), CAT now requires its own separate BFS call in search. It is still O(81) and fast, but it's no longer piggy-backing on move generation. This is the accepted trade-off: movegen perft got ~2× faster; search pays a small fixed cost per node for CAT.

---

## Regression tests

```bash
cd engine
cargo test                          # full suite (debug)
cargo test --release                # faster, release mode
cargo test --release perft_depth4 -- --ignored  # ~3s, checks 247_569_030
```

Key test files:
- `perft.rs` — `PERFT3_STARTPOS`, `PERFT4_STARTPOS`, `perft_depth2/3/4_matches_oracle`
- `test_replay.rs` — `g1v_correctly_rejected_after_replay_prefix` (boundary fix regression)
- `grid.rs` — `flood_layout_centered_with_side_buffers` (layout sanity)
- `path.rs` — `flood_matches_naive_bfs_*` (bitwise ↔ queue BFS agreement on multiple positions)

---

## Architecture snapshot

```
engine/src/
├── board.rs       Board state, make_move/unmake_move, Zobrist hash, Undo
├── grid.rs        O(1) wall checks, can_step, flood_bit_index, FLOOD_STRIDE/PLAYABLE
├── path.rs        DirMasks, bitwise flood fill, BfsScratch, CAT (build_consensus_attention)
├── moves.rs       generate_legal_moves_slice (stack buf), WallPathCache, is_legal_wall
├── search.rs      ID αβ, aspiration windows, LMR table (ln formula), CAT in ordering/LMR
├── perft.rs       perft_fast / perft_fast_ctx (TT), PERFT3/4_STARTPOS constants
└── lib.rs         re-exports (ConsensusAttention, BfsScratch, PERFT4_STARTPOS, …)
```

---

## Next optimization ideas (prioritised)

These are NOT done yet. Prioritised by expected bang-for-buck.

### 1. Incremental DirMasks (high value, medium complexity)
`DirMasks::from_board` iterates all 81 squares and calls `can_step` 4× each = 324 calls per wall trial. Instead:
- Compute masks once for the position.
- When `set_wall` / `unset_wall` is called, touch only the ≤4 squares adjacent to the wall.
- Saves ~300 `can_step` calls per wall candidate in the flood.

### 2. Goal-mask row bitmask shortcut (easy win)
`flood_to_goal` currently checks `component & goal_mask != 0` inside the loop at every level. Instead, break early as soon as the frontier intersects `goal_mask`. This halves the flood for positions where goal row is reachable.

### 3. Make `WallPathCache` path-delta aware for search (medium)
In search, after the best move changes the board, re-use the cached paths rather than rebuilding from scratch. Requires path-delta tracking (which segments are invalidated by which walls). Canta-style.

### 4. CAT re-use across siblings (medium)
CAT is currently rebuilt for every search node. Parent CAT is still valid for all siblings (walls on the same board). Pass CAT down from parent and only rebuild when `make_move` places a wall. Pawn moves never change the BFS graph.

### 5. Parallel perft / search (future)
Not a correctness risk for search (separate `BfsScratch` per thread). Deferred — single-thread correctness first.

### 6. `Move` as `u16` packing (easy, low value)
`Move` is currently 8 bytes (enum). Packing to `u16` (tag + row + col) saves stack in the move buffer and potentially improves cache density. Not the bottleneck.

### 7. Skip BFS entirely for proven-safe walls (research)
If a wall does not pass through any square within BFS distance 2 of either pawn, it cannot form a cage even in theory. Fast geometric pre-filter. Needs careful proof.

---

## What was deliberately NOT touched

| Idea | Why not yet |
|------|-------------|
| Probable-wall pruning | Breaks perft — search only |
| Parallel perft | Correctness debug pain |
| AlphaZero / neural net | Separate large project |
| D* Lite incremental BFS | Profiling first |

---

## Known open questions

1. **Eval quality**: Current eval is `opp_dist - our_dist` centipawns. No wall inventory bonus, no endgame taper, no path-fork detection. Gorisanson still wins games vs Titanium minimax. Main weakness: walls are wasted on positions where shortest-path delta is 0.

2. **LMR tuning**: The `ln(d) * ln(m) / 2.25` formula with `LMR_AFTER_MOVE = 4` was tuned against Gorisanson at 10s. Has not been formally benchmarked across a wider set of positions.

3. **Extensions budget**: Hard ceiling to prevent depth never decreasing in forcing sequences. Value (`MAX_PLY / 4`) chosen conservatively; may need re-tuning.

4. **Perft 5 timing**: ~18s on this machine. Canta/Ishtar references confirm 28,837,934,502 nodes. We match. Not worth further optimising until after search quality improves.

---

## Commands cheatsheet

```bash
# In engine/
cargo test
cargo test --release
cargo test --release perft_depth4 -- --ignored   # locked oracle
cargo run --release -- perft 3
cargo run --release -- perft 4
cargo run --release -- bench 12 10s              # 10s budget, depth 12 search
cargo run --release -- genmove                   # greedy test move

# Benchmark suite
node benchmark/perft_triple.mjs
node benchmark/titanium_vs_gorisanson.mjs --games 20
```

---

## File-level change summary (this session, not yet committed)

```
engine/src/grid.rs        +91   FLOOD_STRIDE/COL_PAD/ROW_PAD, flood_bit_index/sq, FLOOD_PLAYABLE
engine/src/path.rs        +713  DirMasks, expand_frontier, flood_to_goal, BfsScratch, CAT
engine/src/moves.rs       +127  WallPathCache, lazy known-path skip in is_legal_wall
engine/src/search.rs      +35   CAT_HOT_CM/COLD_CM, cat_score_for_move, order uses CAT
engine/src/perft.rs       +12   PERFT4_STARTPOS constant + test
engine/src/lib.rs         +7    re-export ConsensusAttention, BfsScratch, PERFT4_STARTPOS
engine/src/test_replay.rs +27   g1v regression test, old test marked #[ignore]
```
