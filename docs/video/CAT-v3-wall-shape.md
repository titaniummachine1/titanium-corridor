# CAT v3 - Cross-gap wall shape attention

**Episode hook:** Search was pruning useful corridor walls, but the first shape pass over-credited useless T-junctions far from the race.

---

## Problem

1. Opening MCTS loved `e5v` after `e3v d4h` because rollouts saw +1 on Black while ignoring that White's own path also lengthened.
2. The first geometry pass treated any perpendicular-at-chain-end as "half protrusion". That revived futile T-walls on the opposite side of the board and polluted ordering.

---

## Correct geometry

### Cross-gap (tiny ordering nudge only)

Perpendicular wall placed **through the one-row/col gap** between two parallel walls:

- `V(r-1,c) + V(r+1,c)` -> candidate `H(r,c)`
- `H(r,c-1) + H(r,c+1)` -> candidate `V(r,c)`

Adjacent chain ends (`V(r,c)+V(r+1,c)` with `H` at the junction) are **not** cross-gap walls.

### Cross-gap block

Shifted placement beside the door slot that would become a cross-gap:

- vertical gap at col `c` -> `H(r,c-1)` or `H(r,c+1)`
- horizontal gap at row `r` -> `V(r-1,c)` or `V(r+1,c)`

---

## Search integration

| Mechanism | Effect |
|-----------|--------|
| `wall_shape_attention_bonus` | +40 cm cross / +35 cm block in move ordering only |
| Gating | `max(edge heat, touched heat) >= CAT_HOT_CM` (160) |
| Pruning | Shape bonus does **not** rescue otherwise dead walls |

Static eval is unchanged.

---

## Opening fix

Hybrid switches to minimax once `walls_placed >= 2`, so ply 9 after `e3v d4h` no longer uses opening MCTS.

MCTS expansion now drops walls with non-positive net race value (`opp_gain - our_loss <= 0`, or `net < 2` when tied/behind).
