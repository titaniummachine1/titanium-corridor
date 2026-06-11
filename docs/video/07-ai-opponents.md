# Episode 07 — First opponent: Gorisanson (local)

- **branch:** `checkpoint/07-gorisanson-ui`
- **commit:** `7c85a20`
- **tag:** `checkpoint-07-gorisanson-ui`

## Hook

"Perft proved the rules. Now we pick a boss — **gorisanson MCTS** runs locally in the browser and in the terminal. Same AI, two arenas."

## What shipped

| Piece             | Path                                  |
| ----------------- | ------------------------------------- |
| Opponent registry | `web/src/lib/playerRegistry.js`       |
| Local MCTS worker | `web/src/workers/gorisansonWorker.js` |
| Terminal matches  | `benchmark/head_to_head.mjs`          |
| Play API          | `benchmark/lib/gorisanson_ai.mjs`     |

## UI

- Player `<select>` **optgroups**: Human · Local · Remote · Competition (planned)
- Default: **Human vs Gorisanson MCTS**
- **AI time preset** shows budget under dropdown:
  - Intuition → 2,500 sims
  - Short → 7,500
  - Medium → 20,000
  - Long → 60,000
- Remote Ishtar/Ka still show **visit counts** (+ parallelism for Ishtar)

## Terminal (no user input)

```bash
node benchmark/head_to_head.mjs --games 4 --p1 7500 --p2 20000
```

Outputs score + provisional Elo. Use for CI smoke and video "stronger preset wins" segment.

## Next

Episode 08: [08-greedy-ui-lab.md](08-greedy-ui-lab.md) — per-player testing UI, scraped sliders, greedy `genmove`, Titanium vs Gorisanson bench.
