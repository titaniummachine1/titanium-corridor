# Movegen + core handoff

**`main`** — production: shift wall masks + `ShiftCanStep` pawns. No pawn O1 tables.  
**`movgen-o1-lookup`** — research: ~2MB pawn `PAWN_LEGAL` lookup + generator + pawn-only bench.

---

## Done

| Item | Status |
| ---- | ------ |
| **Wall shift L2/TOPO** (`wall_masks.rs`) | ✓ **production** — measured speedup |
| Lazy L3, split loops | ✓ |
| Perft bulk d1, gates exact | ✓ |
| §A Zobrist / slim Undo | ✓ |
| §B pawn default `ShiftCanStep` | ✓ |
| Pawn O1 tables | ✓ **`movgen-o1-lookup` branch only** (~3% pawn-only; not shipped) |
| Pinned bench script | ✓ `scripts/bench-pinned.ps1` |

### Gates

```text
perft 3 = 2_062_264
perft 4 = 247_569_030
cargo test --release → 123 passed (+ 1 ignored d4)
scripts/bench-pinned.ps1 → ~231M nps @ core 7
```

---

## Branch split (important)

| What | `main` | `movgen-o1-lookup` |
| ---- | ------ | ------------------- |
| `wall_masks()` shift path | ✓ | ✓ |
| `ShiftCanStep` pawns | ✓ default | ✓ default |
| `PAWN_LEGAL` + 1.6MB remap | ✗ | ✓ research |
| `movegen-o1-gen` | ✗ | ✓ |
| `perft_pawn_only` bench | ✗ | ✓ |

**Do not confuse:** wall shift masks are the production win. Pawn O1 is the optional gimmick.

---

## Next work

1. L3 flood fraction in **search** replays  
2. §C incremental L3 (needs proof harness)  
3. §D completeness oracle  
4. Eval / search (STATE.md)
