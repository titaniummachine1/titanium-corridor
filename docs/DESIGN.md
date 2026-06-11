# Titanium Engine — design notes

## Hybrid search (planned)

```
Time budget T
├── Phase 1 (~40–50% T): ID + αβ + Zobrist TT + aspiration windows
└── Phase 2 (remainder): MCTS with gorisanson rollouts, seeded from Phase 1 PV
```

## Path / eval

- Dual BFS distance fields per position (cache in TT entry)
- Incremental invalidation on wall placement — D\* Lite only if profiling demands it

## Move ordering (Phase 1 — implemented)

1. TT best move (exact hash match)
2. Pawn steps that shorten `our_dist` to goal
3. Walls that lengthen `opp_dist` (path-delta positive)
4. CAT-hot moves (`CAT_HOT_CM ≥ 180`) — skip LMR, treated as tactical
5. Remaining walls ordered by CAT score (centi-unit tie-breaker)
6. CAT-cold moves (`CAT_COLD_CM < 80`) — get +1 ply extra LMR reduction

LMR table: `floor(0.5 + ln(depth) * ln(moves_searched) / 2.25)`, capped at `depth/2`.  
Full-depth window: first `LMR_AFTER_MOVE = 4` moves per node.

## Pondering (planned — not active)

Stockfish-style: think while opponent moves. Opponent compute is untouched.

| Engine      | Approach                                       | Status                                    |
| ----------- | ---------------------------------------------- | ----------------------------------------- |
| Ishtar / Ka | `go ponder` / `stop` on WebSocket              | `EngineClient.ponder()` ready, not called |
| Local MCTS  | Predicted reply + node-cap search + tree reuse | Blocked on one-shot worker                |

Prep: `docs/video/09-pondering-prep.md`, `web/src/lib/enginePonder.js`, `appController.maybePonderInactiveEngines()`.

Ponder budget: **rollout cap only**, no wall-clock limit.

## External benchmarks

| Opponent          | Role                   |
| ----------------- | ---------------------- |
| JS `gameLogic.js` | Rules oracle           |
| gorisanson MCTS   | Local OSS baseline     |
| Ishtar @ Short    | External strength exam |

## References

- [titaniummachine1/titanium-quoridor](https://github.com/titaniummachine1/titanium-quoridor)
- [gorisanson/quoridor-ai](https://github.com/gorisanson/quoridor-ai)
- [pavlosdais/Quoridor](https://github.com/pavlosdais/Quoridor)
