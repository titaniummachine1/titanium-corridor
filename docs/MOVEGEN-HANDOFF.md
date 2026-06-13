# Movegen handoff → Claude Fable

**Branch after merge:** `main` has closed movegen.  
**Follow-up branch:** `movgen-improvements` for optional perf work.  
**Last verified:** Jun 2026 — 130 tests green, perft gates exact.

---

## Done (do not redo)

| Item | Commit area |
| ---- | ----------- |
| Shift L2 collision masks | `lookup.rs` |
| Shift TOPO flood-skip (`two_of_three`) | `lookup.rs` |
| Lazy `Option<WallTrialCtx>` | `legal.rs` |
| Perft bulk count depth 1 | `util/perft.rs` |
| `wall_masks()` bundle | `lookup.rs` |
| Split isolated / flood wall loops | `legal.rs` |
| Pawn O1 tables (offline gen only) | `build/movegen_o1/`, `o1/tables.rs` |
| Docs | `docs/MOVEGEN.md`, README, STATE |

### Gates (must stay green)

```text
perft 3 = 2_062_264
perft 4 = 247_569_030
cargo test --release → 130 passed
```

### Measured (this machine, release)

```bash
titanium perft 3      # 2_062_264
titanium perft 4      # 247_569_030
titanium bench 3 10   # ~175–250M nps (honest make/unmake)
cargo bench --bench perft_pawn_modes
```

---

## Fable: verify on merge

1. **Audit diff** — any fake O(1) loops, dead table paths, duplicate remap bins?
2. **Run full suite** — commands above
3. **Confirm** `ShiftCanStep` default still correct given O1 bench (see MOVEGEN.md table)
4. **Merge** `movgen-o1-lookup` → `main` if not already done

---

## Fable: next work (priority order)

### A. Make/unmake + Zobrist (highest ROI, not movegen)

**Hypothesis:** make/unmake is now 30–50% of honest bench node cost.

**Tasks:**

- Profile `Board::make_move` / `unmake_move` (flamegraph or `perf record`)
- Slim `Undo` struct — drop unused fields
- Fuse redundant Zobrist xors (wall move does 3+ xors on walls_remaining)
- Re-bench `titanium bench 3 20`

**Risk:** Low — perft gates catch breakage.

---

### B. Pawn O1Lookup decision

**Data:** `cargo bench --bench perft_pawn_modes` — O1 ~11% faster than Shift on perft(4) no-TT; Shift keeps cache clean in combined wall+pawn nodes.

**Tasks:**

- Optional: in-search profile on real replay positions
- If Shift wins in search: feature-gate `O1Lookup` + `movegen-o1-gen` as `research-o1-pawn`
- Document decision in MOVEGEN.md

---

### C. Incremental L3 / witness flood skip (hard)

**Only if** profiling shows L3 dominates in wall-heavy search positions.

**Requirements before code:**

- Proof harness: exhaustive vs scalar `both_players_reach_goals_grids` on wall trials
- Never repeat unsound witness skip (`a1h`/`a5h` — BUG-DIARY)
- Characterize residual family if partial skip only

**Risk:** High — soundness.

---

### D. Completeness program (research)

Not movegen micro-opts. Batch exact oracle:

- Millions of solved states, invariant hash, collision hunt
- Uses engine legality + search — movegen is ready

---

## Intentionally NOT done

- Movegen multithreading
- GPU movegen
- Wall/topo lookup tables
- Pawn `can_step` → bitboard fusion (optional on `movgen-improvements`)
- Incremental reachability in L3
- Merge pawn O1 as production default

---

## Suspected slop (flagged for audit)

- `wall_physically_legal_o1` still exists for tests — production uses masks only
- Orphan remap bins: check `generated_wall_pseudo_*.bin`, `generated_wall_topo_*.bin` if any remain
- `lookup.rs` module comment was stale (fixed in closure commit)
- `docs/video/PERFT-BENCHMARKS.md` old ~3.4s d4 row — updated in README/STATE; video docs may lag

---

## Branch map

| Branch | Purpose |
| ------ | ------- |
| `main` | Production — closed movegen |
| `movgen-improvements` | Optional: pawn bitboard can_step, flood ordering, micro-opts |
| `movgen-o1-lookup` | Historical — merged into main |

---

## Contact protocol

Cursor ships bounded + test-green slices. Fable owns merge audit, proof-heavy algo, and make/unmake. Do not start item C without harness spec signed off.
