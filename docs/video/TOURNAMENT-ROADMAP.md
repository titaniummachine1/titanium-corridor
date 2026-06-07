# Tournament roadmap — version vs version, Elo progress

**Status:** prepared, not built. This doc is the contract for a future episode: _“every Titanium checkpoint fights every other — who’s strongest?”_

---

## Can we do it?

**Yes.** Pattern:

1. Each episode = **git tag + branch + commit** (already in [README.md](README.md)).
2. `git checkout <tag>` → `cargo build --release` → binary `titanium-<episode>`.
3. Round-robin (or Swiss) — each pair plays many games at fixed time.
4. **Elo** (or Glicko-2) from results → leaderboard → proof of progress.

Same idea as chess engine testing (CEGT/STS style), but our stages are **our own checkpoints** plus optional externals (gorisanson, scraped UI bot when wired).

---

## What each stage can do today

| Stage          | Tag / branch                                                | Commit     | Can play? | Tournament role                                           |
| -------------- | ----------------------------------------------------------- | ---------- | --------- | --------------------------------------------------------- |
| 01 BFS         | `checkpoint-01-path-bfs` / `checkpoint/01-path-bfs`         | `43a1b93`  | No search | Perft/oracle only — **skip** head-to-head until `genmove` |
| 02 Moves       | `checkpoint-02-legal-moves`                                 | `19864b8`  | No search | Same                                                      |
| 03 Perft       | `checkpoint-03-perft`                                       | `5a4b0fc`  | No search | Same                                                      |
| 04 Bench       | `checkpoint-04-bench`                                       | `90193b0`  | No search | Same                                                      |
| 05 Bugfix      | `checkpoint-05-perft-bugfix` / `checkpoint/05-perft-bugfix` | `6b9e00d`  | No search | Perft gate = **2_062_264** @ d3                           |
| 06 Thread prep | `checkpoint-06-threading`                                   | `098477c`  | No search | `thread-bench` only                                       |
| 07 Gorisanson  | `checkpoint-07-gorisanson-ui` / `checkpoint/07-gorisanson-ui` | `7c85a20`  | MCTS UI   | Local opponent + `head_to_head.mjs`                       |
| 08+ αβ search  | `checkpoint-08-alphabeta` …                                 | _(future)_ | **Yes**   | **First real Titanium Elo ladder**                        |
| main           | `main`                                                      | moving     | Latest    | Always “current champ”                                    |

**Rule:** No Elo between checkpoints until at least one has **`genmove`** (αβ or MCTS). Before that, compare with **perft**, **nps**, **thread-bench** — not playing strength.

---

## Checkpoint discipline (do at every episode commit)

```bash
git checkout -b checkpoint/NN-short-name
# ... work ...
git commit -m "checkpoint NN: one-line why"
git tag checkpoint-NN-short-name
git rev-parse --short HEAD   # paste into README + episode .md
```

Fill in three places:

1. `docs/video/README.md` — table row
2. `docs/video/NN-*.md` — `branch`, `commit`, `tag` header
3. **This file** — tournament registry row + “Can play?” column

Optional fourth: `benchmark/checkpoints.json` (future) — machine-readable list for automation.

---

## Future tournament design (when search exists)

### Contestants

| ID              | Source                        | Notes                             |
| --------------- | ----------------------------- | --------------------------------- |
| `titanium-07`   | tag `checkpoint-07-alphabeta` | First internal entrant            |
| `titanium-08`   | tag `checkpoint-08-…`         | + move ordering, etc.             |
| `titanium-main` | `main` @ CI hash              | Rolling latest                    |
| `gorisanson`    | `_vendor/quoridor-mcts`       | External baseline (MCTS)          |
| `pavlosdais`    | C binary if built             | External baseline (αβ) — optional |
| `human`         | UI                            | Manual — not for auto ladder      |

### Protocol

- **Time control:** e.g. 3s/move (matches `perft-race` story).
- **Games:** round-robin, 2 games per pair (colors swap) × N contestants.
- **Opening:** startpos only at first; later add position suite.
- **Result:** win / loss / draw (draw rare in Quoridor but possible with rules).
- **Rating:** Elo 1500 start; K=32 for first 30 games per engine, then K=16.

### CLI (implemented / planned)

```bash
# Gorisanson vs Gorisanson — different sim counts (terminal, no browser)
node benchmark/head_to_head.mjs
node benchmark/head_to_head.mjs --games 10 --p1 7500 --p2 20000
node benchmark/head_to_head.mjs --games 1 --verbose   # show moves + AI logs

# Future: build all tagged binaries, full round-robin
node benchmark/build_checkpoints.mjs
node benchmark/tournament.mjs --time 3 --games 20 --engines titanium-07,gorisanson
```

**Web UI:** Player combo → Gorisanson MCTS (local) · Ishtar/Ka (remote) · Titanium/pavlosdais (soon). Time preset shows budget (`~7,500 MCTS rollouts` or `~3,200 visits`).

### Worktree trick (avoid constant checkout)

```bash
git worktree add ../titanium-cp07 checkpoint-07-alphabeta
git worktree add ../titanium-cp08 checkpoint-08-…
# build each once, keep binaries side by side
```

---

## Progress metrics before Elo (now → episode 06)

| Metric      | Command                           | What it proves              |
| ----------- | --------------------------------- | --------------------------- |
| Correctness | `cargo test` + `perft_triple.mjs` | Rules match JS / gorisanson |
| Speed       | `bench 3 20`                      | NPS trend across commits    |
| Depth       | `perft-race 3`                    | Max depth in 3s wall clock  |
| Parallel    | `thread-bench 4`                  | Titanium vs Titanium cores  |
| Divide      | `perft_diff.mjs`                  | Regression hunter           |

Save numbers in episode notes or a future `benchmark/history.json` when we automate.

---

## Video episode arc (future)

1. **“The ladder”** — build 3–4 tagged binaries, run overnight tournament.
2. **“Elo board”** — table on screen: 07 &lt; 08 &lt; main, gorisanson line.
3. **“Regression”** — one checkpoint loses to an older one → bug or eval mistake → BUG-DIARY entry.

Hook line: _“Every commit is a fighter. Tags are its name. Elo is the scoreboard.”_

---

## Registry (update at each checkpoint)

| Ep  | Tag                          | Commit    | Branch                       | Play?   | Notes                 |
| --- | ---------------------------- | --------- | ---------------------------- | ------- | --------------------- |
| 01  | `checkpoint-01-path-bfs`     | `43a1b93` | `checkpoint/01-path-bfs`     | No      | BFS only              |
| 02  | `checkpoint-02-legal-moves`  | `19864b8` | `checkpoint/02-legal-moves`  | No      | Move gen              |
| 03  | `checkpoint-03-perft`        | `5a4b0fc` | `checkpoint/03-perft`        | No      | Divide                |
| 04  | `checkpoint-04-bench`        | `90193b0` | `checkpoint/04-bench`        | No      | Criterion             |
| 05  | `checkpoint-05-perft-bugfix` | _TBD_     | `checkpoint/05-perft-bugfix` | No      | d8v fix, d3=2_062_264 |
| 06  | `checkpoint-06-threading`    | _TBD_     | `checkpoint/06-threading`    | No      | Engine + thread-bench |
| 07  | `checkpoint-07-alphabeta`    | _future_  | `checkpoint/07-alphabeta`    | **Yes** | First Elo tournament  |

---

## Related

- [README.md](README.md) — episode scripts + commits
- [PERFT-BENCHMARKS.md](PERFT-BENCHMARKS.md) — speed gates
- [REFERENCES.md](../REFERENCES.md) — external opponents
- [06-threading-prep.md](06-threading-prep.md) — latest stage before search
