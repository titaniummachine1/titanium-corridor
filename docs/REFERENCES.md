# External engine references (ideas only)

## Scraped JS — **correctness oracle**

| Path                       | Role                                  |
| -------------------------- | ------------------------------------- |
| `web/src/lib/gameLogic.js` | Perft oracle, move labels, wall rules |

## pavlosdais/Quoridor (C) — competition-style engine

- **Repo:** `_vendor/pavlosdais-quoridor` (clone of [github.com/pavlosdais/Quoridor](https://github.com/pavlosdais/Quoridor))
- **Protocol:** [quoridor.di.uoa.gr QTP](http://quoridor.di.uoa.gr/qtp/qtp.html) — `playmove`, `playwall`, `genmove`
- **Move gen:** `src/generate_moves.c` — unrolled pawn jumps + wall loops
- **Walls:** `wallAbove` / `wallBelow` / `wallOnTheRight` / `wallOnTheLeft` in `src/utilities.c`
- **Search:** αβ + iterative deepening + Zobrist TT (`src/engine.c`, `zobrist_hashing.c`)
- **Path:** BFS (`src/bfs.c`) for eval and wall legality
- **No perft** — we built our own; wall helpers confirm lateral geometry

## gorisanson/quoridor-ai (JS MCTS)

- **Repo:** `_vendor/quoridor-mcts` — [github.com/gorisanson/quoridor-ai](https://github.com/gorisanson/quoridor-ai)
- **Role:** Secondary perft oracle (`benchmark/lib/gorisanson_moves.mjs` — full legal moves, not MCTS probable-wall subset)
- **Agrees with scraped JS + Rust** at perft depth 3: **2_062_264** nodes (standard gate)
- **Search:** guided MCTS (2.5k–60k rollouts/move on website) — different from our αβ plan

## Chess perft tooling (pattern)

- [agausmann/perftree](https://github.com/agausmann/perftree) — divide diff vs Stockfish
- Our `benchmark/perft_diff.mjs` is the Quoridor equivalent (JS oracle instead of Stockfish)
