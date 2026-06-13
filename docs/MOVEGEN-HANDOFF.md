# Movegen + core handoff

**`main` @ `9302db0`** — movegen closed, §A (Zobrist/Undo) merged.  
**Branch `movgen-improvements`** — same as `main` after fast-forward.

---

## Done

| Item | Status |
| ---- | ------ |
| Shift L2 / TOPO wall masks | ✓ production |
| Lazy L3, `wall_masks`, split loops | ✓ |
| Perft bulk d1, gates exact | ✓ |
| §A const Zobrist, fused deltas, slim `Undo` | ✓ merged `main` |
| §B pawn default | ✓ **`ShiftCanStep`** — see MOVEGEN.md “why not O1” |
| Movegen multithread / GPU | ✗ policy: never |

### Gates

```text
perft 3 = 2_062_264
perft 4 = 247_569_030
cargo test --release → 130 passed
titanium bench 3 20 → ~210–240M nps (honest)
```

---

## O1 pawn — research only (not a bug)

**We are not “failing to use” O1 — we chose not to make it default.**

- Tables exist, tests verify them vs scalar.
- `generate_legal_moves_slice` uses `PawnGenMode::default()` → `ShiftCanStep`.
- `O1Lookup` only runs when code passes that mode (perft tests, `perft_pawn_modes` bench).
- Wall production path lives in `lookup.rs` shifts; pawn tables are a separate offline artifact for future completeness work.

---

## Next work (Fable or Cursor)

### 1. L3 flood fraction in **search** (not perft)

Profile wall-heavy replay positions — is §C incremental L3 worth a proof harness?

### 2. §C incremental L3

**Blocked** until harness spec + tests vs scalar flood (BUG-DIARY `a1h`/`a5h`).

### 3. §D completeness oracle

Batch exact solve + invariant hash collisions — research track.

### 4. Eval / search (STATE.md)

Distance cache, opening depth, Game A/B replays.

---

## Do not redo

Movegen tables, topo tables, movegen threads, O1 as default without in-search proof.
