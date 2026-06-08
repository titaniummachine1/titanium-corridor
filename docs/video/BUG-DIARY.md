# Bug diary — plot twists for the video series

Chronological. Each entry: **symptom → cause → fix → lesson**.

---

## 1. Wall count zero at start (128 walls missing)

**Symptom:** `titanium moves` → 3 moves only (pawns). JS has 131.

**Cause:** Inverted `canWallBlock` logic. Scraped JS allows “floating” walls when topology check is false; we **rejected** them.

**Fix:** Only run path BFS when `can_wall_block_topology` is true; otherwise legal if no collision.

**Lesson:** Read the oracle, not your intuition about “useless” walls.

---

## 2. BFS panic on top row

**Symptom:** `path::tests::start_position_reachable` crashed in `has_vertical`.

**Cause:** Sideways step from row 8 used `js_row = 9` for wall lookup.

**Fix:** `has_horizontal` / `has_vertical` return false for out-of-range js_row (open border).

**Lesson:** Board edges are real edge cases — pawn grid is 9×9, wall slots are 8×8.

---

## 3. Perft depth 2 off by 2 nodes (THE first perft bug)

**Symptom:**

```
JS 16677  vs  Rust 16679
perft_diff → only d8v and e8v subtrees differ
```

**Cause:** Wrong vertical wall anchors for **lateral** `can_step`. After `d8v`, Black at `e9` could illegally step to `d9`.

**Fix:** Match scraped `pawnCanMove`:

- Right: check vertical at `from` and one row below `from`
- Left: check vertical at `to` and one row below `to`

**Regression:** `grid::tests::vertical_d8v_blocks_black_left_from_e9`, `perft_depth2_matches_js_oracle` = 16_677.

**Lesson:** Divide first. Two nodes at depth 2 = one pawn move wrong somewhere in the tree.

---

## 4. “Rust is fast… but perft 3 is already the ceiling” (design surprise)

**Symptom:** We picked Rust for speed. Perft 3 finishes in ~0.2s. Perft 4 looks hung.

**Cause:** Quoridor branching is ~**131** at the root, not ~20 like chess. Depth 3 = **2M nodes**. Depth 4 ≈ **100× more** ≈ hundreds of millions of clones + BFS wall checks. **Language speed does not save you** from exponential blow-up.

**What Rust actually bought us:**

- Depth **3** correctness check in a blink (JS oracles struggle at the same depth)
- Headroom for **search** at millions of nodes/sec — if we search _smart_

**What we still need (or perft/search past d3 is pointless):**

1. **Make/unmake** — stop cloning the whole board every node
2. **Tactical wall pruning** — search only walls that change shortest-path (perft keeps full legality)
3. **Zobrist TT** — reuse subtrees (gorisanson/pavlosdais both do this for _play_, not perft)
4. **Aspiration / ID** — don't re-walk the whole tree blind every ply

**Lesson for the video:** “Rust isn’t a cheat code — it’s a bigger engine bay. Past perft 3 you either get smart or you wait forever.”

---

## 5. “Perft 4 takes forever” (not a bug — same root as #4)

**Symptom:** `cargo run -- perft 4` runs minutes / appears hung.

**Cause:** ~2M nodes at depth 3 × ~100+ branching ≈ **hundreds of millions** of nodes. Naive clone-per-node perft.

**Mitigation (done):** make/unmake, Zobrist TT, in-place wall trials, `BfsScratch`, stack move buffer — see `PERFT-OPTIMIZATIONS.md`. Depth 4 now ~7s release.

**Lesson:** Quoridor ≠ chess perft tables. Depth 2 is unit test; depth 3 is correctness gate; depth 4 is stress test.

---

## 6. Shared move buffer panic in fast perft

**Symptom:** `index out of bounds: len is 0 but index is 3` in `perft_fast_ctx`.

**Cause:** `generate_legal_moves_into` clears `ctx.moves` on recursion; parent loop still indexing old length.

**Fix:** Snapshot moves per node — first `mem::take`, then stack `[Move; 140]` via `generate_legal_moves_slice`.

**Lesson:** Reused buffers need explicit ownership boundaries at recursion edges (Stockfish uses stack move lists per frame).

---

## 7. Gorisanson infinite spinner (coordinate bridge)

**Symptom:** Local MCTS never returns; board spinner runs forever.

**Cause:** `gorisansonBridge.js` used `row + 1` instead of flipping rows. UI row 1 = bottom; Gorisanson row 0 = top. Invalid moves → `applyAction` fails → `maybeRequestAiMove` loops.

**Fix:** `PAWN_ROWS - row` (9) for pawns, `WALL_ROWS - row` (8) for walls — same flip in `benchmark/lib/gorisanson_bridge.mjs`.

**Lesson:** Two “standard” coordinate systems on one board — always test one known pawn move (e2 from start) through the full bridge.

---

## 8. Remote engine red `!` after second move

**Symptom:** Ishtar/Ka show error state after human's second ply; WebSocket `log Error` or close.

**Cause:** We sent `makemove` only after **human** plies. Scraped app sends every `takeAction` to all engines — including the AI's own `bestmove`. Server was one ply behind → illegal position on next human move.

**Fix:** `syncRemoteEnginesAfterMove` after human moves **and** after remote AI `onBestMove`.

**Lesson:** Cloud engines are state machines; mirror the scraped sync contract, don't assume `bestmove` updates server memory.

---

## 9. Endgame sideways moves with 0 walls left

**Symptom:** After wall stock hits 0, MCTS sometimes steps sideways or backward despite a clear race to the goal — throws wins or delays losses.

**Cause:** Gorisanson MCTS still picks among pawn children by visit count; rollouts use heuristics that are not pure shortest-path when branching is low.

**Fix (v2):** No hard “walls = 0 → BFS skip.” `gorisanson_search_core.mjs` estimates branching each move from pawn moves + `ourWalls × openSlots × totalWallsLeft` (no wall enumeration). When `b^d` fits ~800k nodes at depth ≥ 8, switch to iterative minimax (BFS distance eval: `opp_dist - our_dist`). Otherwise MCTS.

**Lesson:** Branching is mostly wall inventory and board space left — count that, don’t enumerate. Minimax when the proxy says the tree is small enough.

---

## 10. `test_replay_legality` — external replay used pre-fix rules

**Symptom:** `test_replay::test_replay_legality` failed. Move 24 `g1v` was deemed illegal.

**Cause:** The hardcoded replay came from a game played before the horizontal boundary fix (commit `9e4cbf5`, `js_col == 8`). Under the corrected rules, `g1v` at move 24 blocks a goal path and is correctly rejected.

**Fix:** Marked old test `#[ignore]`. Added `g1v_correctly_rejected_after_replay_prefix` which asserts the wall is **not** in the legal move list.

**Lesson:** External replays can silently embed pre-fix illegal moves. Always isolate the specific move before marking a test wrong.

---

## 11. `expand_frontier_no_row_wrap_east_west` — wrong assertion

**Symptom:** Test asserted flood from `(0,0)` didn't reach `(1,0)` — but of course it does (south step is legal).

**Cause:** Test intent was to verify that an east shift from `(0,8)` (board east edge) doesn't bleed into `(1,0)`. Assertion was testing the wrong thing.

**Fix:** Refactored test to explicitly place a pawn at `(0,8)`, shift east, verify the resulting bit is in the padding column and `flood_sq_from_bit` returns `None` for it.

**Lesson:** "No row wrap" tests must isolate the specific edge square, not test reachability from a corner.

---

## 12. `flood_bit_index` — `const fn` disallows `u32::from(u8)`

**Symptom:** Compile error: `cannot call conditionally-const associated function <u32 as From<u8>>::from`.

**Cause:** `From::from` is not stabilised as `const fn` in Rust stable. `u32::from(row)` inside a `const fn` is rejected.

**Fix:** `row as u32` and `col as u32` casts — explicit type casts are allowed in `const fn`.

**Lesson:** In `const fn` contexts, use `as` casts, not trait-based conversions.

---

## 13. Known-path wall skip — eager cache caused regression

**Symptom:** After adding `WallPathCache`, perft 4 jumped from ~3.0s to ~4.27s.

**Cause:** `WallPathCache::new` was called for every wall candidate because the topology check returned `true` for many floaters. Building both shortest paths (two BFS passes) per position wiped out the BFS-skip savings.

**Fix:** Made `WallPathCache` lazy via `Option<WallPathCache>` with `get_or_insert_with`. Now it is built at most once per position and only when `can_wall_block_topology` has already returned `true` for at least one wall. All subsequent topology-true walls reuse the same cache.

**Lesson:** "Build once per position, not per candidate" only works if initialisation is deferred until the first candidate actually needs it.

---

## Oracle stack (for cross-platform debugging)

1. **Primary:** scraped `web/src/lib/gameLogic.js` (netlify UI rules)
2. **Secondary:** [gorisanson/quoridor-ai](https://github.com/gorisanson/quoridor-ai) full legal moves (`benchmark/lib/gorisanson_moves.mjs`)
3. **Reference:** pavlosdais C wall geometry (`_vendor/pavlosdais-quoridor`) — no perft

All three agree at **perft depth 3** (2_062_264 nodes) after bug #3 fix.
